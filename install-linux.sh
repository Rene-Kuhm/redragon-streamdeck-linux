#!/bin/bash

# =============================================================================
# Redragon Stream Deck Manager - Instalador Universal Linux
# =============================================================================
# Detecta automáticamente la distribución e instala apropiadamente

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

VERSION="2.0.0"
GITHUB_REPO="Rene-Kuhm/redragon-streamdeck-linux-"

print_header() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║     Redragon Stream Deck Manager - Instalador Universal      ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_step() { echo -e "${GREEN}[+]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[!]${NC} $1"; }
print_error() { echo -e "${RED}[✗]${NC} $1"; }
print_success() { echo -e "${GREEN}[✓]${NC} $1"; }

# Detectar distribución
detect_distro() {
    if [ -f /etc/arch-release ]; then
        DISTRO="arch"
    elif [ -f /etc/fedora-release ]; then
        DISTRO="fedora"
    elif [ -f /etc/debian_version ]; then
        DISTRO="debian"
    else
        DISTRO="unknown"
    fi
    
    print_success "Distribución detectada: $DISTRO"
}

# Instalar en Arch
install_arch() {
    print_step "Instalando dependencias para Arch..."
    sudo pacman -S --needed --noconfirm webkit2gtk gtk3 libusb openssl glib2 base-devel ydotool playerctl
    
    setup_common
    build_from_source
}

# Instalar en Fedora
install_fedora() {
    print_step "Instalando dependencias para Fedora..."
    sudo dnf install -y webkit2gtk4.1 gtk3 libusb1 openssl ydotool playerctl curl wget
    
    setup_common
    
    # Intentar usar .rpm si existe
    if [ -f "releases/redragon-streamdeck-${VERSION}-1.x86_64.rpm" ]; then
        print_step "Instalando desde paquete .rpm..."
        sudo dnf install -y "releases/redragon-streamdeck-${VERSION}-1.x86_64.rpm"
    else
        build_from_source
    fi
}

# Instalar en Debian/Ubuntu
install_debian() {
    print_step "Instalando dependencias para Debian/Ubuntu..."
    sudo apt update
    sudo apt install -y libwebkit2gtk-4.1-0 libgtk-3-0 libusb-1.0-0 libssl3 ydotool playerctl curl wget
    
    setup_common
    
    # Intentar usar .deb si existe
    if [ -f "releases/redragon-streamdeck_${VERSION}_amd64.deb" ]; then
        print_step "Instalando desde paquete .deb..."
        sudo dpkg -i "releases/redragon-streamdeck_${VERSION}_amd64.deb" || sudo apt install -f -y
    else
        build_from_source
    fi
}

# Configuración común (udev, ydotool)
setup_common() {
    print_step "Configurando reglas udev..."
    RULES_FILE="/etc/udev/rules.d/99-redragon-streamdeck.rules"
    if [ ! -f "$RULES_FILE" ]; then
        echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666", TAG+="uaccess"' | sudo tee "$RULES_FILE" > /dev/null
        sudo udevadm control --reload-rules
        sudo udevadm trigger
    fi
    
    print_step "Configurando ydotool..."
    sudo systemctl enable ydotoold.service 2>/dev/null || true
    sudo systemctl start ydotoold.service 2>/dev/null || true
    
    if ! groups | grep -q input; then
        sudo usermod -aG input "$USER"
        print_warning "Necesitarás cerrar sesión para que funcionen los hotkeys"
    fi
}

# Compilar desde fuente
build_from_source() {
    print_step "Compilando desde fuente..."
    
    # Instalar Rust si no está
    if ! command -v cargo &>/dev/null; then
        print_step "Instalando Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    print_step "Compilando aplicación (puede tardar unos minutos)..."
    cargo build --release --manifest-path src-tauri/Cargo.toml
    
    sudo cp src-tauri/target/release/redragon-streamdeck /usr/local/bin/
    sudo chmod +x /usr/local/bin/redragon-streamdeck
    
    # Desktop entry
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
EOF
    
    print_success "Aplicación instalada"
}

# Resumen
show_summary() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║              ¡Instalación Completada!                        ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Ejecuta: redragon-streamdeck"
    echo "O búscalo en el menú de aplicaciones"
    echo ""
    echo "Documentación: CLAUDE.md"
    echo ""
    if ! groups | grep -q input; then
        echo -e "${YELLOW}⚠ IMPORTANTE: Cierra sesión y vuelve a iniciar para que funcionen los hotkeys${NC}"
    fi
}

# Main
main() {
    print_header
    detect_distro
    
    case "$DISTRO" in
        arch)
            install_arch
            ;;
        fedora)
            install_fedora
            ;;
        debian)
            install_debian
            ;;
        *)
            print_error "Distribución no soportada automáticamente"
            print_warning "Intenta compilar manualmente con:"
            echo "  cargo build --release --manifest-path src-tauri/Cargo.toml"
            exit 1
            ;;
    esac
    
    show_summary
}

main "$@"
