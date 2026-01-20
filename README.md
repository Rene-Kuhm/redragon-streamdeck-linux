# Redragon Stream Deck Linux

Driver y panel de control open source para **Redragon SS-550 Stream Deck** en Linux.

![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Linux-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.x-blue)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)

## Características

### Funciones Básicas
- Interfaz gráfica nativa (Tauri/GTK)
- Soporte para múltiples páginas de botones
- Iconos personalizados (100x100)
- Ejecución de comandos del sistema
- Control de brillo
- Navegación entre páginas con botones físicos
- Compatible con Wayland (Hyprland, Sway, GNOME) y X11

### Funciones Avanzadas
- **URLs**: Abrir páginas web con un botón
- **Texto**: Escribir texto automáticamente (ydotool)
- **Hotkeys**: Simular atajos de teclado (Ctrl+C, Alt+Tab, etc.)
- **Multi-acción**: Secuencias de comandos con delays

### Widgets Dinámicos (actualización automática)
- **Reloj**: Hora actual (con/sin segundos)
- **Fecha**: Día, mes, año, día de la semana
- **Sistema**: CPU%, RAM%, Temperatura
- **Temporizador**: Cuenta regresiva configurable

### Integraciones de Streaming
- **OBS Studio** (WebSocket 5.x):
  - Iniciar/detener streaming y grabación
  - Cambiar escenas
  - Mutear/desmutear micrófono
  - Widget de estado en tiempo real
- **Twitch API**:
  - Mostrar viewers y followers en botones
  - Crear clips con un clic
  - Correr comerciales
  - Enviar mensajes al chat

## Instalación en Arch Linux

### Método Rápido

```bash
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-
git checkout feature/tauri-desktop-app
chmod +x install.sh
./install.sh
```

### Método Manual

Ver [INSTALL_ARCH.md](INSTALL_ARCH.md) para instrucciones detalladas.

## Uso

### Ejecutar la aplicación

```bash
redragon-streamdeck
```

O busca "Redragon Stream Deck" en el menú de aplicaciones.

### Configurar un botón

1. Haz clic en cualquier botón en la interfaz
2. Configura:
   - **Etiqueta**: Texto que se muestra
   - **Comando**: Acción a ejecutar
   - **Color**: Color de fondo
   - **Icono**: Imagen personalizada

### Comandos Especiales

| Categoría | Comando | Descripción |
|-----------|---------|-------------|
| **Navegación** | `__NEXT_PAGE__` | Página siguiente |
| | `__PREV_PAGE__` | Página anterior |
| | `__PAGE_0__` | Ir a página específica |
| **URLs** | `__URL_https://youtube.com` | Abrir URL |
| **Texto** | `__TYPE_Hola mundo` | Escribir texto |
| **Hotkeys** | `__KEY_ctrl+shift+s` | Simular teclas |
| **Multi** | `__MULTI_cmd1;;cmd2` | Secuencia de comandos |
| **Widgets** | `__CLOCK__` | Reloj HH:MM |
| | `__CPU__` | Uso de CPU |
| | `__RAM__` | Uso de RAM |
| | `__TIMER_5__` | Timer 5 minutos |
| **OBS** | `__OBS_STREAM__` | Toggle streaming |
| | `__OBS_RECORD__` | Toggle grabación |
| | `__OBS_SCENE_Gaming` | Cambiar escena |
| **Twitch** | `__TWITCH_VIEWERS__` | Mostrar viewers |
| | `__TWITCH_CLIP__` | Crear clip |

Ver [CLAUDE.md](CLAUDE.md) para la lista completa de comandos.

## Configurar Integraciones

### OBS Studio

1. En OBS: **Tools > WebSocket Server Settings**
2. Habilitar "Enable WebSocket server"
3. (Opcional) Configurar password

```bash
# Ejecutar con password de OBS
OBS_WEBSOCKET_PASSWORD="tu_password" redragon-streamdeck
```

### Twitch

1. Crear app en https://dev.twitch.tv/console
2. Configurar variables de entorno:

```bash
export TWITCH_CLIENT_ID="tu_client_id"
export TWITCH_ACCESS_TOKEN="tu_token"
export TWITCH_CHANNEL="tu_canal"
```

## Distribución de Botones

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
├── src-tauri/
│   ├── src/lib.rs     # Backend Rust (USB, OBS, Twitch)
│   └── Cargo.toml     # Dependencias Rust
├── public/
│   ├── index.html     # Interfaz gráfica
│   ├── app-tauri.js   # JavaScript frontend
│   └── style.css      # Estilos
├── install.sh         # Instalador Arch Linux
├── uninstall.sh       # Desinstalador
├── INSTALL_ARCH.md    # Guía de instalación detallada
└── CLAUDE.md          # Documentación de comandos
```

## Solución de Problemas

### El Stream Deck no se detecta

```bash
# Verificar conexión
lsusb | grep "0200:1000"

# Verificar reglas udev
cat /etc/udev/rules.d/99-redragon-streamdeck.rules

# Desconectar y reconectar el dispositivo
```

### Los hotkeys no funcionan

```bash
# Verificar ydotoold
systemctl status ydotoold.service

# Verificar grupo input
groups | grep input

# Si no estás en el grupo, agrégarte y reiniciar sesión
sudo usermod -aG input $USER
```

### Error "Interface Busy"

```bash
# Buscar procesos usando el dispositivo
pkill -f redragon

# Reiniciar la aplicación
redragon-streamdeck
```

## Desinstalar

```bash
./uninstall.sh
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
- Desarrollado con ayuda de Claude AI

## Licencia

Este proyecto está bajo la licencia MIT. Ver [LICENSE](LICENSE) para más detalles.

---

⭐ Si este proyecto te fue útil, ¡dale una estrella en GitHub!
