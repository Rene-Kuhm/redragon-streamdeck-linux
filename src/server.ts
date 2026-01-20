import express from "express";
import multer from "multer";
import path from "path";
import fs from "fs";
import { exec, execSync } from "child_process";
import * as usb from "usb";
import { StreamDock } from "./streamdock.js";

const app = express();
const PORT = 3000;

const VID = 0x0200;
const PID = 0x1000;

const ROOT = process.cwd();
const CONFIG_PATH = path.join(ROOT, "config.json");
const ICONS_PATH = path.join(ROOT, "icons");
const PUBLIC_PATH = path.join(ROOT, "public");

if (!fs.existsSync(ICONS_PATH)) {
  fs.mkdirSync(ICONS_PATH, { recursive: true });
}

const storage = multer.diskStorage({
  destination: ICONS_PATH,
  filename: (req, file, cb) => {
    const ext = path.extname(file.originalname);
    const page = req.params.page || "0";
    cb(null, `page${page}_btn${req.params.id}${ext}`);
  },
});
const upload = multer({ storage });

interface ButtonConfig {
  label: string;
  command: string;
  color: string;
  icon: string;
}

interface Page {
  name: string;
  buttons: Record<string, ButtonConfig>;
}

interface Config {
  brightness: number;
  currentPage: number;
  pages: Page[];
}

function loadConfig(): Config {
  return JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8"));
}

function saveConfig(config: Config) {
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2));
}

let streamDock: StreamDock | null = null;
let device: usb.Device | null = null;
let currentConfig: Config = loadConfig();

async function connectStreamDeck() {
  try {
    device = usb.findByIds(VID, PID) || null;
    if (!device) {
      console.log("Stream Deck no conectado");
      return false;
    }

    device.open();
    const iface = device.interfaces?.[0];
    if (!iface) return false;

    if (iface.isKernelDriverActive()) {
      iface.detachKernelDriver();
    }
    iface.claim();

    const inEndpoint = iface.endpoints?.find((ep) => ep.direction === "in") as usb.InEndpoint;
    const outEndpoint = iface.endpoints?.find((ep) => ep.direction === "out") as usb.OutEndpoint;

    if (!inEndpoint || !outEndpoint) return false;

    outEndpoint.transferType = usb.usb.LIBUSB_TRANSFER_TYPE_INTERRUPT;
    inEndpoint.transferType = usb.usb.LIBUSB_TRANSFER_TYPE_INTERRUPT;

    streamDock = new StreamDock({
      send(data: Buffer) {
        return new Promise<void>((resolve, reject) => {
          outEndpoint.transfer(data, (error) => {
            if (error) reject(error);
            else resolve();
          });
        });
      },
      receive(byteSize = 512) {
        return new Promise<Buffer>((resolve, reject) => {
          inEndpoint.transfer(byteSize, (error, data) => {
            if (error) reject(error);
            else resolve(data ?? Buffer.alloc(0));
          });
        });
      },
      controlTransfer(bmRequestType: number, bRequest: number, wValue: number, wIndex: number, wLength: number) {
        return new Promise<Buffer | number | undefined>((resolve, reject) => {
          device!.controlTransfer(bmRequestType, bRequest, wValue, wIndex, wLength, (error, data) => {
            if (error) reject(error);
            else resolve(data);
          });
        });
      },
    });

    await streamDock.wakeScreen();
    console.log("Stream Deck conectado");
    return true;
  } catch (e) {
    console.error("Error conectando Stream Deck:", e);
    return false;
  }
}

async function generateButtonImage(pageIndex: number, keyId: string, button: ButtonConfig): Promise<string> {
  const outputFile = path.join(ICONS_PATH, `generated_p${pageIndex}_${keyId}.png`);

  if (button.icon && fs.existsSync(path.join(ICONS_PATH, button.icon))) {
    return path.join(ICONS_PATH, button.icon);
  }

  const fontSize = button.label.length > 8 ? 10 : button.label.length > 5 ? 12 : 14;
  const cmd = `magick -size 256x256 xc:"${button.color}" -gravity center -pointsize ${fontSize * 2} -fill white -annotate 0 "${button.label}" "${outputFile}" 2>/dev/null || convert -size 256x256 xc:"${button.color}" -gravity center -pointsize ${fontSize * 2} -fill white -annotate 0 "${button.label}" "${outputFile}"`;

  try {
    execSync(cmd, { stdio: "pipe" });
  } catch {
    execSync(`magick -size 256x256 xc:"${button.color}" "${outputFile}" 2>/dev/null || convert -size 256x256 xc:"${button.color}" "${outputFile}"`, { stdio: "pipe" });
  }

  return outputFile;
}

async function loadPage(pageIndex: number) {
  if (!streamDock) return;

  currentConfig = loadConfig();
  if (pageIndex < 0 || pageIndex >= currentConfig.pages.length) {
    pageIndex = 0;
  }

  currentConfig.currentPage = pageIndex;
  saveConfig(currentConfig);

  const page = currentConfig.pages[pageIndex];
  console.log(`Cargando página: ${page.name}`);

  await streamDock.clearScreen();
  await streamDock.setBrightness(Math.floor(currentConfig.brightness * 0.64));

  for (const [keyId, button] of Object.entries(page.buttons)) {
    try {
      const imagePath = await generateButtonImage(pageIndex, keyId, button);
      await streamDock.setKeyImage(parseInt(keyId), imagePath);
    } catch (e) {
      console.error(`Error updating button ${keyId}:`, e);
    }
  }
}

async function handleButtonPress(keyId: number) {
  const page = currentConfig.pages[currentConfig.currentPage];
  const button = page.buttons[keyId.toString()];

  if (!button?.command) return;

  const cmd = button.command;

  // Comandos especiales para navegación de páginas
  if (cmd === "__NEXT_PAGE__") {
    const nextPage = (currentConfig.currentPage + 1) % currentConfig.pages.length;
    await loadPage(nextPage);
    return;
  }

  if (cmd === "__PREV_PAGE__") {
    const prevPage = (currentConfig.currentPage - 1 + currentConfig.pages.length) % currentConfig.pages.length;
    await loadPage(prevPage);
    return;
  }

  const pageMatch = cmd.match(/^__PAGE_(\d+)__$/);
  if (pageMatch) {
    const targetPage = parseInt(pageMatch[1]);
    if (targetPage >= 0 && targetPage < currentConfig.pages.length) {
      await loadPage(targetPage);
    }
    return;
  }

  // Comando normal
  console.log(`→ ${button.label || `Botón ${keyId}`}`);
  exec(cmd, (error) => {
    if (error) console.error(`Error: ${error.message}`);
  });
}

async function startButtonListener() {
  if (!streamDock) return;

  console.log("Escuchando botones...");
  while (streamDock) {
    try {
      const { keyId, state } = await streamDock.receiveKeyPress();
      if (state === 1) {
        await handleButtonPress(keyId);
      }
    } catch (e) {
      console.error("Error leyendo botón:", e);
      break;
    }
  }
}

// Express middleware
app.use(express.json());
app.use(express.static(PUBLIC_PATH));
app.use("/icons", express.static(ICONS_PATH));

// API Routes
app.get("/api/config", (req, res) => {
  res.json(loadConfig());
});

app.post("/api/config", async (req, res) => {
  const config = req.body as Config;
  saveConfig(config);
  currentConfig = config;
  await loadPage(currentConfig.currentPage);
  res.json({ success: true });
});

app.get("/api/page/:index", async (req, res) => {
  const index = parseInt(req.params.index);
  await loadPage(index);
  res.json({ success: true, currentPage: index });
});

app.put("/api/page/:page/button/:id", async (req, res) => {
  const { page, id } = req.params;
  const pageIndex = parseInt(page);
  const buttonConfig = req.body as ButtonConfig;

  const config = loadConfig();
  if (config.pages[pageIndex]) {
    config.pages[pageIndex].buttons[id] = buttonConfig;
    saveConfig(config);
    currentConfig = config;

    if (pageIndex === currentConfig.currentPage) {
      await loadPage(pageIndex);
    }
  }
  res.json({ success: true });
});

app.post("/api/page/:page/button/:id/icon", upload.single("icon"), async (req, res) => {
  const { page, id } = req.params;
  const pageIndex = parseInt(page);

  if (!req.file) {
    return res.status(400).json({ error: "No file uploaded" });
  }

  const config = loadConfig();
  if (config.pages[pageIndex]) {
    config.pages[pageIndex].buttons[id].icon = req.file.filename;
    saveConfig(config);
    currentConfig = config;

    if (pageIndex === currentConfig.currentPage) {
      await loadPage(pageIndex);
    }
  }
  res.json({ success: true, filename: req.file.filename });
});

app.delete("/api/page/:page/button/:id/icon", async (req, res) => {
  const { page, id } = req.params;
  const pageIndex = parseInt(page);

  const config = loadConfig();
  if (config.pages[pageIndex]) {
    const oldIcon = config.pages[pageIndex].buttons[id].icon;
    if (oldIcon) {
      const iconPath = path.join(ICONS_PATH, oldIcon);
      if (fs.existsSync(iconPath)) {
        fs.unlinkSync(iconPath);
      }
    }
    config.pages[pageIndex].buttons[id].icon = "";
    saveConfig(config);
    currentConfig = config;

    if (pageIndex === currentConfig.currentPage) {
      await loadPage(pageIndex);
    }
  }
  res.json({ success: true });
});

app.post("/api/page", async (req, res) => {
  const { name } = req.body;
  const config = loadConfig();

  const newPage: Page = {
    name: name || `Página ${config.pages.length + 1}`,
    buttons: {}
  };

  for (let i = 1; i <= 15; i++) {
    newPage.buttons[i.toString()] = { label: "", command: "", color: "#1a1a2e", icon: "" };
  }

  config.pages.push(newPage);
  saveConfig(config);
  currentConfig = config;

  res.json({ success: true, pageIndex: config.pages.length - 1 });
});

app.delete("/api/page/:index", async (req, res) => {
  const index = parseInt(req.params.index);
  const config = loadConfig();

  if (config.pages.length > 1 && index >= 0 && index < config.pages.length) {
    config.pages.splice(index, 1);
    if (config.currentPage >= config.pages.length) {
      config.currentPage = config.pages.length - 1;
    }
    saveConfig(config);
    currentConfig = config;
    await loadPage(currentConfig.currentPage);
  }

  res.json({ success: true });
});

app.put("/api/page/:index/name", async (req, res) => {
  const index = parseInt(req.params.index);
  const { name } = req.body;
  const config = loadConfig();

  if (config.pages[index]) {
    config.pages[index].name = name;
    saveConfig(config);
    currentConfig = config;
  }

  res.json({ success: true });
});

app.post("/api/brightness", async (req, res) => {
  const { brightness } = req.body;
  const config = loadConfig();
  config.brightness = brightness;
  saveConfig(config);
  currentConfig = config;

  if (streamDock) {
    await streamDock.setBrightness(Math.floor(brightness * 0.64));
  }
  res.json({ success: true });
});

app.get("/api/status", (req, res) => {
  res.json({ connected: streamDock !== null });
});

app.post("/api/reconnect", async (req, res) => {
  const success = await connectStreamDeck();
  if (success) {
    await loadPage(currentConfig.currentPage);
    startButtonListener();
  }
  res.json({ success });
});

// Start server
async function main() {
  console.log("=== Redragon Stream Deck Manager ===\n");

  const connected = await connectStreamDeck();
  if (connected) {
    await loadPage(currentConfig.currentPage);
    startButtonListener();
  }

  app.listen(PORT, () => {
    console.log(`\nInterfaz web: http://localhost:${PORT}`);
  });
}

main().catch(console.error);
