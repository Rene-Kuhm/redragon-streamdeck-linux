#!/bin/bash

# =============================================================================
# Redragon Stream Deck Manager - Instalador para Arch Linux
# =============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║     Redragon Stream Deck Manager - Instalador Arch Linux     ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_step() {
    echo -e "${GREEN}[+]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

# Verificar que estamos en Arch Linux
check_arch() {
    if [ ! -f /etc/arch-release ]; then
        print_error "Este instalador es solo para Arch Linux"
        print_warning "Para otras distros, consulta INSTALL_ARCH.md para instrucciones manuales"
        exit 1
    fi
}

# Instalar dependencias
install_dependencies() {
    print_step "Instalando dependencias del sistema..."

    DEPS="webkit2gtk gtk3 libusb openssl glib2 base-devel ydotool playerctl"

    # Verificar cuáles ya están instaladas
    MISSING=""
    for pkg in $DEPS; do
        if ! pacman -Qi "$pkg" &>/dev/null; then
            MISSING="$MISSING $pkg"
        fi
    done

    if [ -n "$MISSING" ]; then
        echo -e "  Instalando:$MISSING"
        sudo pacman -S --needed --noconfirm $MISSING
    else
        print_success "Todas las dependencias ya están instaladas"
    fi
}

# Configurar ydotool
setup_ydotool() {
    print_step "Configurando ydotool para hotkeys..."

    # Habilitar servicio
    if ! systemctl is-enabled ydotoold.service &>/dev/null; then
        sudo systemctl enable ydotoold.service
    fi

    if ! systemctl is-active ydotoold.service &>/dev/null; then
        sudo systemctl start ydotoold.service
    fi

    # Agregar usuario al grupo input
    if ! groups | grep -q input; then
        print_warning "Agregando usuario al grupo 'input'..."
        sudo usermod -aG input "$USER"
        print_warning "Necesitarás cerrar sesión y volver a iniciar para que los hotkeys funcionen"
    fi

    print_success "ydotool configurado"
}

# Configurar reglas udev
setup_udev() {
    print_step "Configurando reglas udev para el dispositivo USB..."

    RULES_FILE="/etc/udev/rules.d/99-redragon-streamdeck.rules"
    RULES_CONTENT='SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666", TAG+="uaccess"'

    if [ ! -f "$RULES_FILE" ]; then
        echo "$RULES_CONTENT" | sudo tee "$RULES_FILE" > /dev/null
        sudo udevadm control --reload-rules
        sudo udevadm trigger
        print_success "Reglas udev instaladas"
        print_warning "Desconecta y reconecta el Stream Deck"
    else
        print_success "Reglas udev ya existen"
    fi
}

# Verificar/instalar Rust
check_rust() {
    print_step "Verificando Rust..."

    if ! command -v cargo &>/dev/null; then
        print_warning "Rust no está instalado. Instalando..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    print_success "Rust $(cargo --version | cut -d' ' -f2) disponible"
}

# Compilar la aplicación
build_app() {
    print_step "Compilando la aplicación (esto puede tardar unos minutos)..."

    cargo build --release --manifest-path src-tauri/Cargo.toml

    if [ -f "src-tauri/target/release/redragon-streamdeck" ]; then
        print_success "Compilación exitosa"
    else
        print_error "Error en la compilación"
        exit 1
    fi
}

# Instalar la aplicación
install_app() {
    print_step "Instalando la aplicación..."

    # Copiar binario
    sudo cp src-tauri/target/release/redragon-streamdeck /usr/local/bin/
    sudo chmod +x /usr/local/bin/redragon-streamdeck

    # Crear directorio de datos
    mkdir -p ~/.local/share/redragon-streamdeck

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
Categories=Utility;AudioVideo;
Keywords=stream;deck;obs;twitch;
EOF

    # Actualizar base de datos de aplicaciones
    update-desktop-database ~/.local/share/applications/ 2>/dev/null || true

    print_success "Aplicación instalada en /usr/local/bin/redragon-streamdeck"
}

# Configuración opcional de auto-inicio
setup_autostart() {
    echo ""
    read -p "¿Deseas que la aplicación inicie automáticamente? [y/N] " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        print_step "Configurando auto-inicio..."

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

        systemctl --user daemon-reload
        systemctl --user enable redragon-streamdeck.service

        print_success "Auto-inicio configurado"
        print_warning "El servicio iniciará en el próximo login"
    fi
}

# Mostrar resumen final
show_summary() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║              ¡Instalación Completada!                        ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Para ejecutar la aplicación:"
    echo -e "  ${BLUE}redragon-streamdeck${NC}"
    echo ""
    echo "O búscala en el menú de aplicaciones como 'Redragon Stream Deck'"
    echo ""
    echo "Comandos especiales disponibles:"
    echo "  - Widgets: __CLOCK__, __CPU__, __RAM__, __TEMP__"
    echo "  - OBS: __OBS_STREAM__, __OBS_RECORD__, __OBS_SCENE_nombre"
    echo "  - Twitch: __TWITCH_VIEWERS__, __TWITCH_CLIP__"
    echo ""
    echo "Para más información, consulta:"
    echo "  - CLAUDE.md (lista completa de comandos)"
    echo "  - INSTALL_ARCH.md (configuración de OBS/Twitch)"
    echo ""

    if ! groups | grep -q input; then
        echo -e "${YELLOW}⚠ IMPORTANTE: Cierra sesión y vuelve a iniciar para que los hotkeys funcionen${NC}"
        echo ""
    fi
}

# Main
main() {
    print_header

    check_arch
    install_dependencies
    setup_ydotool
    setup_udev
    check_rust
    build_app
    install_app
    setup_autostart
    show_summary
}

# Ejecutar si no se está sourcing
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
