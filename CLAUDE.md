# Redragon Stream Deck — contexto para Claude

Driver y panel de control para el **Redragon SS-550 Stream Deck** en Linux.
El aparato es USB `0200:1000` y en `lsusb` aparece como "TP-Link 9B540": es un
identificador prestado, no es un TP-Link.

## Estructura

Es un **workspace de cargo con tres crates**. El split se hizo para que el
dispositivo se pueda manejar sin entorno gráfico: el daemon no enlaza ninguna
biblioteca de webkit ni de gtk.

```
crates/core      redragon-core       toda la lógica: USB, widgets, OBS, Twitch.
                                     No depende de Tauri.
crates/daemon    redragon-daemon     ejecutable sin interfaz, usa core
src-tauri        redragon-streamdeck la GUI: sólo los #[tauri::command] y el arranque
public/                              el frontend
```

`rdev` (atajos globales de teclado) se quedó a propósito en la GUI: necesita
display, y meterlo en el core reintroduciría justo la dependencia que el split
saca de encima. El daemon no lo necesita, tiene los botones físicos.

### El frontend son dos interfaces distintas

| Archivo | Para qué |
|---|---|
| `public/index.html` + `app-tauri.js` + `ui-shell.js` + `style.css` | La GUI de Tauri. **Es la que se usa.** |
| `public/index-web.html` + `app.js` + `style-web.css` | Cliente del servidor Node (`src/server.ts`, Express) |

**Las dos son reales, no borres ninguna por parecer duplicada.** La variante
web habla por HTTP con `src/server.ts`, que implementa `/api/*` y sirve
`public/` como estático; tiene sus propias dependencias (`usb`, `jimp`). No
está documentada en el README y no se toca desde el lanzamiento inicial, pero
funciona. Cada interfaz tiene su hoja de estilos, así que se pueden cambiar por
separado.

`ui-shell.js` es sólo presentación —la biblioteca de acciones, la paginación,
los paneles laterales, la selección de tecla— y **envuelve** funciones de
`app-tauri.js` sin modificarlas. Si reestructuras el HTML, respeta los IDs y las
funciones globales que engancha el `onclick`: no hay comprobación automática y
lo que se rompe falla en silencio.

## Comandos

```bash
cargo build --release                      # todo el workspace
cargo build --release -p redragon-daemon   # sólo el daemon
cargo test --workspace

./install.sh                  # instalador con detección de distro
./install.sh --daemon-only    # sin GUI ni dependencias gráficas
```

El binario queda en **`target/release/`**, en la raíz del workspace — *no* en
`src-tauri/target/`, aunque compiles apuntando a ese paquete. Es un error fácil:
el actualizador automático estuvo roto justamente por buscarlo en el sitio viejo.

**Compilar no alcanza para probar contra el aparato**: hay que instalar el
binario donde lo arranca el servicio (`~/.local/bin/redragon-streamdeck`) y
reiniciar `redragon-streamdeck.service` (unidad de usuario). El orden importa:
`stop` → instalar → `reset-failed` → `start`. Sin el `reset-failed`, el
`StartLimitBurst=3` de la unidad deja el servicio muerto tras varios reinicios
seguidos y el aparato entero deja de responder.

Para ver los `DEBUG:` hay que definir `REDRAGON_STREAMDECK_DEBUG=1`; en el
servicio, con un drop-in en `~/.config/systemd/user/redragon-streamdeck.service.d/`.

## Ramas y releases

Se trabaja con `feat/*` y `fix/*` y pull request a `main`, que exige PR: **no se
puede pushear directo**. El CI compila en Fedora, Ubuntu, Debian y Arch, más un
job sin DejaVu. `fmt` y `clippy` son informativos a propósito (`continue-on-error`)
porque el árbol arrastra warnings previos.

**Al mergear a `main`, el CI publica el release solo.** La versión sale de los
prefijos de los commits: `feat` sube la minor, `fix`/`perf`/`refactor` la patch,
y `!` o el marcador de ruptura como pie de mensaje sube la major. Si sólo hubo
`docs`, `ci` o `chore`, no publica nada.

**La versión la manda la etiqueta de git, no `Cargo.toml`.** `build.rs` graba la
etiqueta más cercana en `REDRAGON_RELEASE_TAG` y la app compara eso contra el
último release publicado. Tiene que ser así porque el CI no puede escribir en
`main` para bumpear `Cargo.toml`; si la versión viviera ahí quedaría congelada
mientras las etiquetas avanzan, y quien actualizara entraría en un bucle
infinito: recompila y su binario sigue anunciándose con la versión anterior.
Por lo mismo el actualizador clona la etiqueta publicada, no la punta de `main`.

## Comandos especiales de los botones

Un solo guion bajo al final, no dos. Si el comando no empieza con `__` se
ejecuta como shell, o sea que acepta `||` y demás.

### Básicos
| Comando | Formato | Ejemplo |
|---------|---------|---------|
| URL | `__URL_direccion` | `__URL_https://youtube.com` |
| Texto | `__TYPE_texto` | `__TYPE_Hola mundo` |
| Hotkey | `__KEY_teclas` | `__KEY_ctrl+shift+s` |
| Multi-acción | `__MULTI_cmd1;;cmd2` | `__MULTI_firefox;;__DELAY_2000;;__KEY_ctrl+t` |
| Delay | `__DELAY_ms` | `__DELAY_1000` (sólo dentro de MULTI) |
| Página siguiente | `__NEXT_PAGE__` | |
| Página anterior | `__PREV_PAGE__` | |
| Ir a página N | `__PAGE_N__` | `__PAGE_0__` (índice base 0) |

`__PAGE_N__` lleva **dos** guiones bajos al final: la comprobación es
`starts_with("__PAGE_") && ends_with("__")`, así que con uno solo el token es
inválido y no hace nada.

### Widgets (se refrescan solos, ~1 s, e ignoran el icono)
| Comando | Descripción |
|---------|-------------|
| `__CLOCK__` / `__CLOCK_S__` | Hora HH:MM / con segundos |
| `__DATE__` / `__DATE_FULL__` / `__WEEKDAY__` | Fecha y día |
| `__CPU__` / `__RAM__` / `__TEMP__` | Uso de CPU, memoria, temperatura |
| `__TIMER_N__` | Temporizador de N minutos |
| `__WEATHER__` | Clima (Open-Meteo) |
| `__OBS_STATUS__` | Estado de OBS (LIVE/REC) |
| `__TWITCH_VIEWERS__` / `__TWITCH_FOLLOWERS__` | Estadísticas de Twitch |
| `__SPOTIFY__` | Reproduciendo ahora (vía playerctl) |

La temperatura se lee eligiendo el sensor **por nombre de driver hwmon**
(`coretemp`, `k10temp`, …), no por índice: `thermal_zone0` y `hwmon0` son la CPU
en unas máquinas y el chipset o la WiFi en otras. Los nombres pueden traer
sufijo de instancia (`acpitz_0`), así que la comparación lo tolera.

### OBS Studio y Twitch
| Comando | Descripción |
|---------|-------------|
| `__OBS_STREAM__` / `__OBS_RECORD__` / `__OBS_MUTE__` | Alternar emisión, grabación, micrófono |
| `__OBS_SCENE_Gaming` | Cambiar a la escena "Gaming" |
| `__TWITCH_CLIP__` | Crear un clip |
| `__TWITCH_AD_30__` | Publicidad de 30 s (también 60, 90) |
| `__TWITCH_CHAT_Hola!` | Enviar un mensaje al chat |
| `__SPOTIFY_TOGGLE__` | Reproducir o pausar |

### Teclas para `__KEY_`
Modificadores `ctrl shift alt super/win/meta rctrl rshift ralt` · `f1`–`f12` ·
`esc tab enter space backspace delete insert home end pageup pagedown` ·
flechas `up down left right` · `a`–`z` · `0`–`9` ·
multimedia `volumeup volumedown mute playpause next prev` ·
numpad `kp0`–`kp9 kpenter kpplus kpminus kpmultiply kpdivide kpdot`

Los atajos se envían con **ydotool**, que necesita `ydotoold` corriendo. El
socket se busca primero en `$XDG_RUNTIME_DIR` y después en `/tmp`, y si
`YDOTOOL_SOCKET` ya viene del entorno se respeta.

## Configuración

`config.json` e `icons/` viven en
`~/.local/share/com.tecnodespegue.redragon-streamdeck/`.
**La app reescribe `config.json` al cerrarse**, así que hay que editarlo con el
servicio parado o se pisa.

El campo `icon` acepta el nombre del archivo (se busca en `icons/`) o una ruta
absoluta. **No** funciona sin extensión ni como `data:` en base64. La app guarda
cualquier cadena sin normalizar, así que mirar cómo quedó el config no dice si
el formato es válido: hay que mirar el aparato.

### Integraciones, por variables de entorno
```bash
OBS_WEBSOCKET_URL=ws://localhost:4455   # opcional
OBS_WEBSOCKET_PASSWORD=…                # si OBS tiene contraseña
TWITCH_CLIENT_ID=…
TWITCH_ACCESS_TOKEN=…                   # scopes: channel:manage:broadcast,
TWITCH_CHANNEL=…                        # clips:edit, chat:edit
```
En OBS: Herramientas → Ajustes del servidor WebSocket → activarlo.
Los tokens de Twitch se sacan de https://dev.twitch.tv/console.

## Cosas que ya costaron un rato

- **`pkill -f` con la ruta del binario se mata a sí mismo**, porque el patrón
  aparece en la línea de comandos del propio shell. Usar `pkill -x
  redragon-stream`: `comm` viene truncado a 15 caracteres.
- **La regla udev tiene que ir con prefijo `60-`**. Con `99-` se evalúa después
  de `73-seat-late.rules`, que es quien aplica `uaccess`, así que el tag nunca
  tenía efecto y sólo funcionaba por el `MODE="0666"`.
- **Al medir en el journal, acotar con un timestamp tomado justo antes**, no con
  `--since "1 minute ago"`: si el servicio se reinició hace poco se mezclan
  líneas del binario anterior y los conteos salen mal.
- **Si el aparato deja de mandar eventos de tecla**, no lo arregla reiniciar el
  servicio ni un reset por software: hay que desenchufar el USB unos segundos.
  Descartar eso antes de diagnosticar cualquier otra cosa.
- Los fallos de CI por **timeout bajando imágenes de Docker Hub** son
  transitorios: `gh run rerun <id> --failed`, sin tocar el workflow.

## Pendientes conocidos

- **Cambiar de página no redibuja el aparato.** Al pulsar `__NEXT_PAGE__`,
  `__PREV_PAGE__` o `__PAGE_N__` la app cambia `currentPage` y lo persiste, pero
  no vuelve a empujar las imágenes: las pantallas quedan congeladas en la página
  anterior. El único momento en que dibuja las 15 teclas es al arrancar. El
  dispatch y el mapeo de índices están bien —el resto de los botones funciona—,
  y forzando `currentPage` a mano y arrancando sí muestra la página correcta, o
  sea que el problema está en la ruta de refresco.
- Los nombres de paquetes de **openSUSE no tienen cobertura de CI** y están
  marcados así en el código: Leap y Tumbleweed difieren en varios.
- Sacar el `continue-on-error` de `fmt` y `clippy` cuando se limpien los
  warnings que arrastra el árbol.
