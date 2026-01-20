# Redragon Stream Deck Linux

Driver y panel de control open source para **Redragon SS-550 Stream Deck** en Linux.

![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Linux-blue)
![Node](https://img.shields.io/badge/node-%3E%3D18-brightgreen)

## Características

- Interfaz web moderna para configurar botones
- Soporte para múltiples páginas/escenas
- Iconos personalizados (256x256)
- Ejecución de comandos del sistema
- Control de brillo
- Navegación entre páginas con botones físicos
- Auto-inicio con systemd
- Compatible con Hyprland, GNOME, KDE y otros

## Requisitos

- Linux (probado en Arch/CachyOS con Hyprland)
- Node.js 18 o superior
- ImageMagick (para generar imágenes de botones)

## Instalación

### 1. Clonar el repositorio

```bash
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-
```

### 2. Instalar dependencias

```bash
npm install
```

### 3. Configurar permisos USB

Crear regla udev para acceder al dispositivo sin root:

```bash
sudo nano /etc/udev/rules.d/99-redragon-streamdeck.rules
```

Agregar esta línea:

```
SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666"
```

Recargar reglas:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

**Importante:** Desconecta y vuelve a conectar el Stream Deck.

### 4. Crear configuración inicial

```bash
cp config.example.json config.json
```

### 5. Crear carpeta de iconos

```bash
mkdir -p icons
```

### 6. Ejecutar

```bash
npm start
```

Abre tu navegador en: **http://localhost:3000**

## Auto-inicio (systemd)

Para que inicie automáticamente al encender Linux:

### 1. Crear servicio

```bash
mkdir -p ~/.config/systemd/user
nano ~/.config/systemd/user/redragon-streamdeck.service
```

Contenido:

```ini
[Unit]
Description=Redragon Stream Deck Manager
After=graphical-session.target

[Service]
Type=simple
WorkingDirectory=/ruta/a/redragon-streamdeck-linux-
ExecStart=/usr/bin/node node_modules/.bin/tsx src/server.ts
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

**Importante:** Cambia `/ruta/a/redragon-streamdeck-linux-` por la ruta real donde clonaste el repo.

### 2. Activar servicio

```bash
systemctl --user daemon-reload
systemctl --user enable redragon-streamdeck.service
systemctl --user start redragon-streamdeck.service
```

### 3. Verificar estado

```bash
systemctl --user status redragon-streamdeck.service
```

## Uso

### Interfaz Web

1. Abre **http://localhost:3000** en tu navegador
2. Haz clic en cualquier botón para editarlo
3. Configura:
   - **Etiqueta**: Texto que se muestra en el botón
   - **Comando**: Comando a ejecutar (ej: `firefox`, `spotify`)
   - **Color**: Color de fondo del botón
   - **Icono**: Imagen PNG de 256x256 píxeles

### Comandos Especiales

Para navegar entre páginas usa estos comandos especiales:

| Comando | Acción |
|---------|--------|
| `__NEXT_PAGE__` | Ir a la página siguiente |
| `__PREV_PAGE__` | Ir a la página anterior |
| `__PAGE_0__` | Ir a la página 0 (primera) |
| `__PAGE_1__` | Ir a la página 1 |
| `__PAGE_N__` | Ir a la página N |

### Ejemplos de Comandos

```bash
# Abrir aplicaciones
firefox
spotify
code
thunar

# Comandos con fallback
code || codium
kitty || alacritty

# Control de volumen (PipeWire/WirePlumber)
wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+
wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-
wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle

# Hyprland workspaces
hyprctl dispatch workspace 1
hyprctl dispatch workspace 2

# Screenshots
grim -g "$(slurp)" ~/Pictures/screenshot.png
```

## Distribución de Botones

El Stream Deck tiene 15 botones distribuidos así:

```
┌────┬────┬────┬────┬────┐
│ 11 │ 12 │ 13 │ 14 │ 15 │  ← Fila superior
├────┼────┼────┼────┼────┤
│  6 │  7 │  8 │  9 │ 10 │  ← Fila media
├────┼────┼────┼────┼────┤
│  1 │  2 │  3 │  4 │  5 │  ← Fila inferior
└────┴────┴────┴────┴────┘
```

## Estructura del Proyecto

```
redragon-streamdeck-linux/
├── src/
│   ├── server.ts       # Servidor Express + lógica del Stream Deck
│   └── streamdock.ts   # Driver USB de bajo nivel
├── public/
│   ├── index.html      # Interfaz web
│   ├── style.css       # Estilos
│   └── app.js          # JavaScript del frontend
├── icons/              # Iconos de botones (256x256)
├── config.json         # Tu configuración personal
├── config.example.json # Configuración de ejemplo
└── package.json
```

## Solución de Problemas

### El Stream Deck no se detecta

1. Verifica que esté conectado:
```bash
lsusb | grep 0200:1000
```

2. Verifica permisos:
```bash
ls -la /dev/bus/usb/*/*
```

3. Reinstala reglas udev y reconecta el dispositivo.

### Error LIBUSB_ERROR_BUSY

Otro programa está usando el dispositivo. Cierra Wine, Bottles o cualquier otro software que pueda estar accediendo al USB.

### Los iconos no se muestran

- Asegúrate de que sean imágenes de 256x256 píxeles
- Formatos soportados: PNG, JPG
- Verifica que ImageMagick esté instalado: `magick --version`

### El servicio no inicia

Revisa los logs:
```bash
journalctl --user -u redragon-streamdeck.service -f
```

## Dispositivos Compatibles

- Redragon SS-550 (USB ID: 0200:1000)
- Posiblemente otros dispositivos basados en StreamDock/Mirabox

## Contribuir

¡Las contribuciones son bienvenidas!

1. Fork el repositorio
2. Crea una rama: `git checkout -b mi-feature`
3. Haz commit: `git commit -m 'Agregar feature'`
4. Push: `git push origin mi-feature`
5. Abre un Pull Request

## Créditos

- **Tecnodespegue** - Desarrollo y mantenimiento
- Basado en el protocolo de [mirabox-streamdock-node](https://github.com/nicross/mirabox-streamdock-node)

## Licencia

Este proyecto está bajo la licencia MIT. Ver [LICENSE](LICENSE) para más detalles.

---

⭐ Si este proyecto te fue útil, ¡dale una estrella en GitHub!
