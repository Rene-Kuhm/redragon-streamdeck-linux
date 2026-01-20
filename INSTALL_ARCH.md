# Instalación en Arch Linux

Guía completa para instalar Redragon Stream Deck Manager en Arch Linux.

## Requisitos Previos

### 1. Instalar dependencias del sistema

```bash
# Dependencias básicas para Tauri/GTK
sudo pacman -S --needed \
    webkit2gtk \
    gtk3 \
    libusb \
    openssl \
    glib2 \
    base-devel \
    git

# Para simular teclado en Wayland (requerido para hotkeys)
sudo pacman -S --needed ydotool

# Para reproducción multimedia (opcional, para controles de media)
sudo pacman -S --needed playerctl

# Para controles de audio PipeWire (opcional)
sudo pacman -S --needed pipewire-pulse wireplumber
```

### 2. Configurar ydotool (importante para Wayland)

ydotool necesita el servicio ydotoold corriendo:

```bash
# Habilitar e iniciar el servicio
sudo systemctl enable ydotoold.service
sudo systemctl start ydotoold.service

# Agregar tu usuario al grupo input (necesario para ydotool)
sudo usermod -aG input $USER

# Cerrar sesión y volver a iniciar para aplicar cambios de grupo
```

### 3. Configurar reglas udev para el dispositivo USB

Crear el archivo de reglas:

```bash
sudo tee /etc/udev/rules.d/99-redragon-streamdeck.rules << 'EOF'
# Redragon SS-550 Stream Deck
SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666", TAG+="uaccess"
EOF

# Recargar reglas
sudo udevadm control --reload-rules
sudo udevadm trigger
```

**Nota:** Desconecta y vuelve a conectar el Stream Deck después de esto.

## Instalación

### Opción A: Usar el script de instalación (recomendado)

```bash
# Clonar el repositorio
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-

# Cambiar a la rama de la app Tauri
git checkout feature/tauri-desktop-app

# Ejecutar el instalador
chmod +x install.sh
./install.sh
```

### Opción B: Instalación manual

```bash
# Clonar repositorio
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-
git checkout feature/tauri-desktop-app

# Instalar Rust si no lo tienes
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Compilar
cargo build --release --manifest-path src-tauri/Cargo.toml

# Instalar
sudo cp src-tauri/target/release/redragon-streamdeck /usr/local/bin/
sudo chmod +x /usr/local/bin/redragon-streamdeck

# Copiar recursos
mkdir -p ~/.local/share/redragon-streamdeck
cp -r public/* ~/.local/share/redragon-streamdeck/

# Crear entrada de escritorio
mkdir -p ~/.local/share/applications
cat > ~/.local/share/applications/redragon-streamdeck.desktop << 'EOF'
[Desktop Entry]
Name=Redragon Stream Deck
Comment=Control your Redragon SS-550 Stream Deck
Exec=redragon-streamdeck
Icon=input-gaming
Terminal=false
Type=Application
Categories=Utility;
EOF
```

## Configuración de Integraciones (Opcional)

### OBS Studio

1. Abrir OBS Studio
2. Ir a **Tools > WebSocket Server Settings**
3. Habilitar "Enable WebSocket server"
4. Configurar password si lo deseas

Para usar OBS, ejecuta la app con las variables de entorno:

```bash
# Sin password
redragon-streamdeck

# Con password
OBS_WEBSOCKET_PASSWORD="tu_password" redragon-streamdeck
```

O agrega a tu `.bashrc` / `.zshrc`:
```bash
export OBS_WEBSOCKET_PASSWORD="tu_password"
```

### Twitch

1. Crear aplicación en https://dev.twitch.tv/console
2. Obtener Client ID
3. Generar Access Token con estos scopes:
   - `channel:manage:broadcast`
   - `clips:edit`
   - `chat:edit`
   - `chat:read`
   - `channel:read:subscriptions`

```bash
# Configurar variables de entorno
export TWITCH_CLIENT_ID="tu_client_id"
export TWITCH_ACCESS_TOKEN="tu_access_token"
export TWITCH_CHANNEL="tu_canal"
```

## Auto-inicio (Opcional)

### Usando systemd (usuario)

```bash
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/redragon-streamdeck.service << 'EOF'
[Unit]
Description=Redragon Stream Deck Manager
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/redragon-streamdeck
Restart=on-failure
RestartSec=5
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
EOF

# Habilitar
systemctl --user enable redragon-streamdeck.service
systemctl --user start redragon-streamdeck.service
```

## Solución de Problemas

### El dispositivo no se detecta

1. Verifica que las reglas udev están instaladas:
   ```bash
   cat /etc/udev/rules.d/99-redragon-streamdeck.rules
   ```

2. Verifica que el dispositivo está conectado:
   ```bash
   lsusb | grep "0200:1000"
   ```

3. Desconecta y reconecta el dispositivo

### Los hotkeys no funcionan

1. Verifica que ydotoold está corriendo:
   ```bash
   systemctl status ydotoold.service
   ```

2. Verifica que tu usuario está en el grupo input:
   ```bash
   groups | grep input
   ```

3. Si usas X11 en lugar de Wayland, puede que necesites xdotool:
   ```bash
   sudo pacman -S xdotool
   ```

### Error "Interface Busy"

Otro programa está usando el dispositivo USB. Verifica si hay procesos:

```bash
# Buscar procesos usando el dispositivo
lsof /dev/bus/usb/*/* 2>/dev/null | grep -i stream

# Matar procesos si es necesario
pkill -f redragon
```

### OBS no conecta

1. Verifica que OBS está corriendo
2. Verifica que el WebSocket server está habilitado en OBS
3. Verifica el password si lo configuraste

## Desinstalación

```bash
# Eliminar binario
sudo rm /usr/local/bin/redragon-streamdeck

# Eliminar configuración
rm -rf ~/.local/share/redragon-streamdeck
rm -rf ~/.config/redragon-streamdeck

# Eliminar entrada de escritorio
rm ~/.local/share/applications/redragon-streamdeck.desktop

# Eliminar servicio systemd (si lo configuraste)
systemctl --user stop redragon-streamdeck.service
systemctl --user disable redragon-streamdeck.service
rm ~/.config/systemd/user/redragon-streamdeck.service

# Eliminar reglas udev
sudo rm /etc/udev/rules.d/99-redragon-streamdeck.rules
sudo udevadm control --reload-rules
```

## Soporte

- **Issues:** https://github.com/Rene-Kuhm/redragon-streamdeck-linux-/issues
- **Documentación:** Ver `CLAUDE.md` para lista completa de comandos
