# Redragon Stream Deck Linux

Driver y panel de control open source para **Redragon SS-550 Stream Deck** en Linux.

**Funciona en cualquier distribución de Linux.** No está atado a ninguna: se
compila desde el código y habla con el dispositivo por USB, así que corre lo
mismo en Fedora, Debian, Arch, openSUSE, Gentoo, Void, NixOS o la que uses. Lo
único que cambia de una a otra es cuánto trabajo te ahorra el instalador — ver
[Instalación](#instalación).

![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Linux-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.x-blue)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)

## Características

### Funciones Básicas
- Interfaz gráfica nativa (Tauri/GTK), organizada en tres columnas: páginas a
  la izquierda, la grilla de teclas al centro y una biblioteca de acciones
  buscable a la derecha
- Soporte para múltiples páginas de botones
- Iconos personalizados (100x100)
- Ejecución de comandos del sistema
- Control de brillo
- Navegación entre páginas con botones físicos
- **Cerrar la ventana no cierra la aplicación**: se oculta en la bandeja y el
  dispositivo sigue atendido
- Compatible con Wayland (Hyprland, Sway, GNOME) y X11

### Funciones Avanzadas
- **URLs**: Abrir páginas web con un botón
- **Texto**: Escribir texto automáticamente (ydotool)
- **Hotkeys**: Simular atajos de teclado (Ctrl+C, Alt+Tab, etc.)
- **Atajos globales**: Disparar un botón desde el teclado, sin tocar el aparato
- **Multi-acción**: Secuencias de comandos con delays
- **Perfiles por aplicación**: Cambiar de página automáticamente según la
  aplicación que esté en primer plano

### Widgets Dinámicos (actualización automática)
- **Reloj**: Hora actual (con/sin segundos)
- **Fecha**: Día, mes, año, día de la semana
- **Sistema**: CPU%, RAM%, Temperatura
- **Temporizador**: Cuenta regresiva configurable
- **Clima**: Temperatura y estado actual (Open-Meteo)
- **Música**: Lo que está sonando, vía playerctl

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

## Instalación

```bash
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux-.git
cd redragon-streamdeck-linux-
./install.sh
```

El instalador detecta la distribución leyendo `/etc/os-release`, instala las
dependencias, compila, configura la regla udev y ydotool, y deja la aplicación
lista para arrancar sola al iniciar sesión.

**En cualquier distribución funciona; lo que cambia es cuánto hace el
instalador por vos.**

| Distribución | Qué hace falta |
|---|---|
| Fedora/RHEL, Debian/Ubuntu, Arch, openSUSE | Nada: `./install.sh` y listo |
| Derivados (Mint, Pop!\_OS, CachyOS, EndeavourOS, Nobara…) | Nada: se reconocen solos por `ID_LIKE` |
| Cualquier otra (Gentoo, Void, NixOS, Slackware…) | Instalar las dependencias a mano y usar `./install.sh --skip-deps` |

Las dependencias son las mismas en todos lados, sólo cambian los nombres de los
paquetes: compilador de C y Rust, `libusb`, GTK 3 y WebKit2GTK 4.1 para la
interfaz, `librsvg` y appindicator para el icono de bandeja, OpenSSL, una fuente
DejaVu para el texto de las teclas, y `ydotool` y `playerctl` para los atajos y
el widget de música. Si tu distribución no está en la lista, el instalador te lo
dice y te indica cómo seguir, en vez de fallar sin explicación.

### Si tu sistema no usa systemd

El arranque automático se configura con unidades de usuario de systemd. En
distribuciones con otro init (runit, OpenRC, s6…) la aplicación funciona igual
—no depende de systemd para nada de lo suyo— pero el arranque lo configurás vos:

```bash
./install.sh --no-autostart
```

Después lanzás `redragon-streamdeck` como prefieras: con el autoarranque de tu
escritorio, un script de sesión o un servicio de tu propio init.

### Opciones

| Opción | Efecto |
|---|---|
| `--build-only` | Solo compila, no toca el sistema |
| `--skip-deps` | No instala dependencias del sistema |
| `--no-autostart` | No configura el arranque automático |
| `--daemon-only` | Solo el daemon, sin interfaz gráfica |

### Uso sin entorno gráfico

El proyecto se divide en un daemon que maneja el dispositivo y una interfaz
gráfica opcional para configurarlo. El daemon no depende de Tauri, GTK ni
webkit, así que funciona en un equipo sin escritorio, por SSH o en un servidor:

```bash
./install.sh --daemon-only
```

Ambos comparten el mismo `config.json`, de modo que podés configurar los botones
con la aplicación en un equipo y después dejar corriendo solo el daemon. No
pueden usar el dispositivo a la vez: los units de systemd lo declaran con
`Conflicts=`.

```bash
systemctl --user disable --now redragon-streamdeck   # apagar la interfaz
systemctl --user enable  --now redragon-daemon       # usar solo el daemon
```

## Uso

### Ejecutar la aplicación

```bash
redragon-streamdeck
```

O busca "Redragon Stream Deck" en el menú de aplicaciones.

Cerrar la ventana **no** cierra la aplicación: se oculta en la bandeja y el
Stream Deck sigue funcionando. Para volver a abrirla, o para salir de verdad,
usá el menú del icono de la bandeja. Hace falta porque el hilo que atiende los
botones vive en el proceso de la interfaz.

### Auto-inicio

Para iniciar la app automáticamente al iniciar sesión:

```bash
./install.sh --enable-autostart
```

Comandos útiles:

```bash
./install.sh --autostart-status
./install.sh --disable-autostart
```

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

Es un workspace de cargo con tres crates. La separación existe para que el
dispositivo se pueda manejar sin entorno gráfico: el daemon no enlaza webkit ni
GTK.

```
redragon-streamdeck-linux/
├── crates/
│   ├── core/            # redragon-core: USB, widgets, OBS, Twitch. Sin Tauri
│   └── daemon/          # redragon-daemon: ejecutable sin interfaz
├── src-tauri/
│   └── src/lib.rs       # La interfaz: comandos de Tauri y arranque
├── public/
│   ├── index.html       # Interfaz de escritorio (Tauri)
│   ├── app-tauri.js     #   lógica y puente con el backend
│   ├── ui-shell.js      #   presentación: biblioteca de acciones, paneles
│   ├── style.css        #   estilos
│   ├── index-web.html   # Interfaz del servidor Node (ver abajo)
│   ├── app.js           #   su cliente HTTP
│   └── style-web.css    #   sus estilos
├── src/server.ts        # Servidor Express opcional, alternativa a Tauri
├── install.sh           # Instalador (detecta la distribución)
├── uninstall.sh         # Desinstalador
├── redragon-streamdeck.service  # Unidad systemd de la interfaz
├── redragon-daemon.service      # Unidad systemd del daemon
└── CLAUDE.md            # Notas técnicas y de mantenimiento
```

**Hay dos interfaces y las dos funcionan.** La de Tauri es la que instala
`install.sh` y la que se usa normalmente. La otra es un cliente web servido por
`src/server.ts` (Express, con sus propias dependencias `usb` y `jimp`), que se
levanta con `npm start`. Cada una tiene su hoja de estilos, así que se pueden
modificar por separado.

El binario compilado queda en **`target/release/`**, en la raíz del workspace,
no dentro de `src-tauri/`.

## Actualizaciones

La aplicación avisa sola cuando hay una versión nueva. En **Ajustes → Acerca de**
se ve la versión instalada, la revisión y si estás al día; la versión también
queda a la vista en la barra de título.

Cuando hay una disponible, el botón de instalar descarga la versión publicada,
la compila y reemplaza el binario en marcha, dejando una copia `.bak` de la
anterior. Si la aplicación corre como servicio de usuario, se encarga de pararlo
y volver a arrancarlo.

Del lado del repositorio, los releases se publican solos: al mergear a `main`,
si las builds de las cuatro distribuciones pasan, se etiqueta y se publica una
versión nueva. El número sale de los prefijos de los commits — `feat` sube la
minor, `fix` la patch — y los cambios que sólo tocan documentación o CI no
generan versión.

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

# Verificar que el socket sea accesible por el grupo input
stat -c '%a %U:%G %n' /tmp/.ydotool_socket

# Si no estás en el grupo, agrégarte y reiniciar sesión
sudo usermod -aG input $USER
```

El socket de `ydotoold` debe verse así:

```text
660 root:input /tmp/.ydotool_socket
```

Si aparece como `root:root`, reinstala o reinicia el servicio generado por `install.sh`:

```bash
sudo cp ydotoold.service /etc/systemd/system/ydotoold.service
sudo systemctl daemon-reload
sudo systemctl restart ydotoold.service
```

También puedes probar el acceso directamente:

```bash
YDOTOOL_SOCKET=/tmp/.ydotool_socket ydotool type ""
YDOTOOL_SOCKET=/tmp/.ydotool_socket ydotool key 29:1 29:0
```

### Error "Interface Busy"

Significa que otro proceso ya tiene el dispositivo tomado: `claim_interface` es
exclusivo, así que sólo uno puede manejarlo a la vez. Lo habitual es que hayan
quedado dos instancias, o que estén corriendo la interfaz y el daemon juntos.

```bash
# Ver si el servicio ya lo está manejando
systemctl --user status redragon-streamdeck

# Reiniciarlo, que es lo que corresponde si está gestionado por systemd
systemctl --user restart redragon-streamdeck
```

Si quedaron procesos sueltos, fuera de systemd:

```bash
pkill -x redragon-stream
```

> **No uses `pkill -f`** con la ruta del binario: el patrón aparece en la línea
> de comandos del propio shell y termina matando cosas que no querías. El `-x`
> compara contra el nombre del proceso, que el kernel trunca a 15 caracteres —
> de ahí que el patrón vaya cortado.

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
