#!/usr/bin/env bash
#
# Instalador de Redragon Stream Deck.
#
# Reemplaza a install-linux.sh, install-fedora.sh, install-ubuntu.sh y a las
# instrucciones sueltas de INSTALL_ARCH.md. Cuatro scripts paralelos divergen
# por construccion, y ya habian divergido: install-fedora.sh convivio con
# codigo que solo compilaba en Arch. Agregar una distro ahora es agregar una
# fila en packages_for(), no un archivo mas.

set -euo pipefail

readonly BIN_NAME="redragon-streamdeck"
readonly DAEMON_NAME="redragon-daemon"
readonly INSTALL_DIR="/usr/local/bin"
readonly UDEV_RULE="/etc/udev/rules.d/60-redragon-streamdeck.rules"
readonly USB_VENDOR="0200"
readonly USB_PRODUCT="1000"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR

DISTRO_FAMILY=""
PKG_INSTALL=""
SKIP_DEPS=0
BUILD_ONLY=0
NO_AUTOSTART=0
DAEMON_ONLY=0

# ---------------------------------------------------------------- presentacion

if [ -t 1 ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_RED=$'\033[31m'
    C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
else
    C_RESET=""; C_BOLD=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""
fi

step() { printf '%s==>%s %s\n' "$C_BLUE$C_BOLD" "$C_RESET" "$*"; }
ok()   { printf '%s  ok%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn() { printf '%s  !!%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
fail() { printf '%serror%s %s\n' "$C_RED$C_BOLD" "$C_RESET" "$*" >&2; exit 1; }

show_help() {
    cat <<EOF
Instalador de Redragon Stream Deck

  ./install.sh [opciones]

Opciones:
  --build-only     Solo compila; no instala nada ni toca el sistema.
  --skip-deps      No instala dependencias del sistema (ya las tenes).
  --no-autostart   No configura el arranque automatico con systemd.
  --daemon-only    Instala solo el daemon sin interfaz y sin sus dependencias
                   graficas. Para equipos sin entorno de escritorio.
  -h, --help       Esta ayuda.

Distros soportadas: Fedora/RHEL, Debian/Ubuntu, Arch y openSUSE,
mas sus derivados via ID_LIKE de /etc/os-release.
EOF
}

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --build-only)   BUILD_ONLY=1 ;;
            --skip-deps)    SKIP_DEPS=1 ;;
            --no-autostart) NO_AUTOSTART=1 ;;
            --daemon-only)  DAEMON_ONLY=1 ;;
            -h|--help)      show_help; exit 0 ;;
            *)              fail "opcion desconocida: $1 (usa --help)" ;;
        esac
        shift
    done
}

# -------------------------------------------------------------------- deteccion

detect_distro() {
    [ -r /etc/os-release ] || fail "no existe /etc/os-release; distro no reconocible"
    # shellcheck disable=SC1091
    . /etc/os-release

    # ID_LIKE hace que los derivados (Mint, Pop!_OS, EndeavourOS, Nobara,
    # CachyOS...) funcionen sin listarlos uno por uno.
    local id
    for id in ${ID:-} ${ID_LIKE:-}; do
        case "$id" in
            fedora|rhel|centos)  DISTRO_FAMILY="fedora" ;;
            debian|ubuntu)       DISTRO_FAMILY="debian" ;;
            arch|archlinux)      DISTRO_FAMILY="arch" ;;
            opensuse*|suse|sles) DISTRO_FAMILY="suse" ;;
            *)                   continue ;;
        esac
        break
    done

    [ -n "$DISTRO_FAMILY" ] || fail \
"distro no soportada: ${PRETTY_NAME:-${ID:-desconocida}}.
       Instala las dependencias a mano y volve a correr con --skip-deps."

    case "$DISTRO_FAMILY" in
        fedora) PKG_INSTALL="dnf install -y" ;;
        debian) PKG_INSTALL="apt-get install -y --no-install-recommends" ;;
        arch)   PKG_INSTALL="pacman -S --needed --noconfirm" ;;
        suse)   PKG_INSTALL="zypper install -y" ;;
    esac

    ok "Detectado ${PRETTY_NAME:-$ID} (familia: $DISTRO_FAMILY)"
}

# La tabla. Una distro nueva es una entrada mas aca y nada mas.
packages_for() {
    case "$1" in
        fedora)
            echo "gcc gcc-c++ make pkgconf-pkg-config git curl
                  webkit2gtk4.1-devel gtk3-devel libusb1-devel openssl-devel
                  librsvg2-devel libappindicator-gtk3-devel
                  dejavu-sans-fonts ydotool playerctl"
            ;;
        debian)
            echo "build-essential pkg-config git curl ca-certificates
                  libwebkit2gtk-4.1-dev libgtk-3-dev libusb-1.0-0-dev libssl-dev
                  librsvg2-dev libayatana-appindicator3-dev
                  fonts-dejavu-core ydotool playerctl"
            ;;
        arch)
            echo "base-devel pkgconf git curl
                  webkit2gtk-4.1 gtk3 libusb openssl
                  librsvg libayatana-appindicator
                  ttf-dejavu ydotool playerctl"
            ;;
        suse)
            # Sin cobertura en CI: los nombres de openSUSE cambian entre Leap y
            # Tumbleweed. Si alguno falla, --skip-deps e instalar a mano.
            echo "gcc gcc-c++ make pkg-config git curl
                  webkit2gtk3-devel gtk3-devel libusb-1_0-devel libopenssl-devel
                  librsvg-devel libayatana-appindicator3-devel
                  dejavu-fonts ydotool playerctl"
            ;;
    esac
}

# ------------------------------------------------------------------------ sudo

need_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        command -v sudo >/dev/null || fail "hace falta sudo (o correr como root)"
        sudo "$@"
    fi
}

# ------------------------------------------------------------------------ pasos

install_dependencies() {
    if [ "$SKIP_DEPS" -eq 1 ]; then
        warn "--skip-deps: no se instalan dependencias del sistema"
        return
    fi

    step "Instalando dependencias del sistema"

    local packages
    packages=$(packages_for "$DISTRO_FAMILY" | tr -s '[:space:]' ' ')

    if [ "$DAEMON_ONLY" -eq 1 ]; then
        # El daemon no enlaza webkit ni gtk; instalarlos seria traer media
        # pila grafica a un equipo que quiza ni tenga escritorio.
        packages=$(printf '%s' "$packages" | tr ' ' '\n' \
            | grep -vE 'webkit|gtk|appindicator|librsvg' | tr '\n' ' ')
        warn "--daemon-only: se omiten las dependencias graficas"
    fi

    [ "$DISTRO_FAMILY" = "debian" ] && need_root apt-get update

    # shellcheck disable=SC2086
    need_root $PKG_INSTALL $packages

    ok "Dependencias instaladas"
}

setup_udev() {
    step "Instalando la regla udev del dispositivo"

    # TAG+="uaccess" le da acceso al usuario de la sesion local activa, que es
    # exactamente lo que hace falta. No se usa MODE="0666": eso dejaria el
    # dispositivo escribible por cualquier usuario del sistema sin ganar nada.
    #
    # El prefijo es 60- para que corra ANTES de 73-seat-late.rules, que es quien
    # aplica uaccess. Una regla 99- se evalua despues y el tag queda sin efecto.
    local rule
    rule="SUBSYSTEM==\"usb\", ATTR{idVendor}==\"$USB_VENDOR\", ATTR{idProduct}==\"$USB_PRODUCT\", TAG+=\"uaccess\""

    if [ -f "$UDEV_RULE" ] && [ "$(cat "$UDEV_RULE")" = "$rule" ]; then
        ok "La regla udev ya estaba puesta"
    else
        printf '%s\n' "$rule" | need_root tee "$UDEV_RULE" >/dev/null
        need_root udevadm control --reload-rules
        need_root udevadm trigger
        ok "Regla udev instalada en $UDEV_RULE"
        warn "Desconecta y volve a conectar el Stream Deck para que tome efecto"
    fi

    # Reglas de versiones anteriores que dejaban el dispositivo world-writable.
    local legacy
    for legacy in /etc/udev/rules.d/99-redragon-streamdeck.rules \
                  /etc/udev/rules.d/99-redragon.rules; do
        if [ -f "$legacy" ]; then
            need_root rm -f "$legacy"
            warn "Eliminada regla anterior con MODE=0666: $legacy"
        fi
    done
}

setup_ydotool() {
    step "Configurando ydotool (necesario para las acciones de teclado)"

    if ! command -v ydotoold >/dev/null; then
        warn "ydotoold no esta instalado; las acciones de teclado no van a andar"
        return
    fi

    # El socket va en /run y no en /tmp: /tmp es escribible por todos y varias
    # distros lo limpian solas. La app respeta YDOTOOL_SOCKET del entorno.
    local socket="/run/ydotoold/socket"

    need_root tee /etc/systemd/system/ydotoold.service >/dev/null <<EOF
[Unit]
Description=ydotoold - ydotool daemon
After=multi-user.target

[Service]
Type=simple
RuntimeDirectory=ydotoold
RuntimeDirectoryMode=0755
ExecStart=/usr/bin/ydotoold --socket-path=$socket --socket-perm=0660
ExecStartPost=/bin/sh -c 'for i in \$(seq 1 25); do [ -S $socket ] && chgrp input $socket && exit 0; sleep 0.2; done; exit 1'
Restart=on-failure
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF

    need_root systemctl daemon-reload
    need_root systemctl enable ydotoold.service
    need_root systemctl restart ydotoold.service

    # La app lee YDOTOOL_SOCKET del entorno; sin esto buscaria en las rutas por
    # defecto y no encontraria este socket.
    mkdir -p "$HOME/.config/environment.d"
    printf 'YDOTOOL_SOCKET=%s\n' "$socket" \
        > "$HOME/.config/environment.d/60-redragon-ydotool.conf"

    if ! id -nG "$USER" | grep -qw input; then
        need_root usermod -aG input "$USER"
        warn "Te agregue al grupo 'input': cerra sesion y volve a entrar"
    fi

    ok "ydotool configurado (socket en $socket)"
}

check_rust() {
    if command -v cargo >/dev/null; then
        ok "Rust presente: $(cargo --version)"
        return
    fi

    step "Instalando Rust"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    ok "Rust instalado"
}

build_app() {
    if [ "$DAEMON_ONLY" -eq 1 ]; then
        step "Compilando el daemon"
        ( cd "$SCRIPT_DIR" && cargo build --release -p redragon-daemon )
        ok "Compilado"
        return
    fi

    step "Compilando (tarda unos minutos)"
    ( cd "$SCRIPT_DIR" && cargo build --release --workspace )
    ok "Compilado"
}

install_app() {
    local target="$SCRIPT_DIR/target/release"

    step "Instalando"

    # El daemon se instala siempre: es lo que maneja el dispositivo y funciona
    # igual en un equipo sin entorno grafico.
    [ -f "$target/$DAEMON_NAME" ] || fail "no existe $target/$DAEMON_NAME; corre el script sin --build-only"
    need_root install -Dm755 "$target/$DAEMON_NAME" "$INSTALL_DIR/$DAEMON_NAME"
    ok "Daemon instalado en $INSTALL_DIR/$DAEMON_NAME"

    if [ "$DAEMON_ONLY" -eq 1 ]; then
        return
    fi

    local built="$target/$BIN_NAME"
    [ -f "$built" ] || fail "no existe $built; corre el script sin --build-only"
    need_root install -Dm755 "$built" "$INSTALL_DIR/$BIN_NAME"

    mkdir -p "$HOME/.local/share/applications"
    cat > "$HOME/.local/share/applications/$BIN_NAME.desktop" <<EOF
[Desktop Entry]
Name=Redragon Stream Deck
Comment=Control your Redragon SS-550 Stream Deck
Exec=$BIN_NAME
Icon=input-gaming
Terminal=false
Type=Application
Categories=Utility;AudioVideo;
Keywords=stream;deck;obs;twitch;
EOF
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

    ok "Instalado en $INSTALL_DIR/$BIN_NAME"
}

setup_autostart() {
    if [ "$NO_AUTOSTART" -eq 1 ]; then
        warn "--no-autostart: no se configura el arranque automatico"
        return
    fi

    step "Configurando arranque automatico"
    mkdir -p "$HOME/.config/systemd/user"

    # Los dos units se instalan, pero solo se habilita uno: el dispositivo no
    # puede estar tomado por la GUI y el daemon a la vez (Conflicts= lo declara).
    install -Dm644 "$SCRIPT_DIR/$DAEMON_NAME.service" \
        "$HOME/.config/systemd/user/$DAEMON_NAME.service"

    if [ "$DAEMON_ONLY" -eq 1 ]; then
        systemctl --user daemon-reload
        systemctl --user enable "$DAEMON_NAME.service"
        ok "El daemon arranca solo al iniciar sesion"
        return
    fi

    install -Dm644 "$SCRIPT_DIR/$BIN_NAME.service" \
        "$HOME/.config/systemd/user/$BIN_NAME.service"
    systemctl --user daemon-reload
    systemctl --user enable "$BIN_NAME.service"
    ok "Arranca solo al iniciar sesion"
}

show_summary() {
    printf '\n%sRedragon Stream Deck listo.%s\n\n' "$C_GREEN$C_BOLD" "$C_RESET"
    cat <<EOF
  Ejecutar:     $BIN_NAME
  Iniciar:      systemctl --user start $BIN_NAME
  Ver logs:     journalctl --user -u $BIN_NAME -f
  Diagnostico:  REDRAGON_STREAMDECK_DEBUG=1 $BIN_NAME

  Sin interfaz: systemctl --user disable --now $BIN_NAME
                systemctl --user enable --now $DAEMON_NAME

  Si es la primera instalacion, desconecta y volve a conectar el dispositivo.
EOF
}

main() {
    parse_args "$@"

    printf '\n%sRedragon Stream Deck - instalador%s\n\n' "$C_BOLD" "$C_RESET"

    if [ "$BUILD_ONLY" -eq 1 ]; then
        check_rust
        build_app
        ok "Binario en src-tauri/target/release/$BIN_NAME"
        exit 0
    fi

    detect_distro
    install_dependencies
    check_rust
    build_app
    install_app
    setup_udev
    setup_ydotool
    setup_autostart
    show_summary
}

main "$@"
