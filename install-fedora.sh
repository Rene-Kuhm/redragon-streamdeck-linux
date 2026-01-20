#!/bin/bash

# =============================================================================
# Redragon Stream Deck Manager - Instalador para Fedora
# =============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_header() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║       Redragon Stream Deck Manager - Instalador Fedora       ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_step() { echo -e "${GREEN}[+]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[!]${NC} $1"; }
print_error() { echo -e "${RED}[✗]${NC} $1"; }
print_success() { echo -e "${GREEN}[✓]${NC} $1"; }

# Verificar distro
check_distro() {
    if [ ! -f /etc/fedora-release ]; then
        print_error "Este instalador es para Fedora"
        exit 1
    fi
    print_success "Detectado: $(cat /etc/fedora-release)"
}

# Instalar dependencias
install_dependencies() {
    print_step "Instalando dependencias..."
    
    sudo dnf install -y \
        webkit2gtk4.1 \
        gtk3 \
        libusb1 \
        openssl \
        ydotool \
        playerctl \
        curl \
        wget
    
    print_success "Dependencias instaladas"
}

# Configurar ydotool
setup_ydotool() {
    print_step "Configurando ydotool..."

    # Crear servicio ydotoold si no existe
    SERVICE_FILE="/etc/systemd/system/ydotoold.service"
    if [ ! -f "$SERVICE_FILE" ] && [ ! -f "/usr/lib/systemd/system/ydotoold.service" ]; then
        print_warning "Creando servicio ydotoold.service..."
        sudo tee "$SERVICE_FILE" > /dev/null << 'EOF'
[Unit]
Description=ydotoold - ydotool daemon
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/bin/ydotoold
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF
        sudo systemctl daemon-reload
    fi

    # Habilitar servicio
    sudo systemctl enable ydotoold.service 2>/dev/null || true
    sudo systemctl start ydotoold.service 2>/dev/null || true

    # Agregar usuario al grupo input
    if ! groups | grep -q input; then
        sudo usermod -aG input "$USER"
        print_warning "Necesitas cerrar sesión para que los hotkeys funcionen"
    fi

    print_success "ydotool configurado"
}

# Configurar udev
setup_udev() {
    print_step "Configurando reglas udev..."
    
    RULES_FILE="/etc/udev/rules.d/99-redragon-streamdeck.rules"
    
    if [ ! -f "$RULES_FILE" ]; then
        echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666", TAG+="uaccess"' | sudo tee "$RULES_FILE" > /dev/null
        sudo udevadm control --reload-rules
        sudo udevadm trigger
        print_warning "Desconecta y reconecta el Stream Deck"
    fi
    
    print_success "Reglas udev configuradas"
}

# Instalar paquete .rpm
install_rpm() {
    print_step "Instalando aplicación..."
    
    RPM_FILE="releases/redragon-streamdeck-2.0.0-1.x86_64.rpm"
    
    if [ -f "$RPM_FILE" ]; then
        sudo dnf install -y "$RPM_FILE"
        print_success "Aplicación instalada desde paquete .rpm"
    else
        print_warning "Paquete .rpm no encontrado, compilando desde fuente..."
        install_from_source
    fi
}

# Compilar desde fuente
install_from_source() {
    print_step "Instalando dependencias de compilación..."
    
    sudo dnf groupinstall -y "Development Tools"
    sudo dnf install -y \
        webkit2gtk4.1-devel \
        openssl-devel \
        libusb1-devel \
        gtk3-devel
    
    print_step "Instalando Rust..."
    
    if ! command -v cargo &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    print_step "Compilando (esto puede tardar)..."
    cargo build --release --manifest-path src-tauri/Cargo.toml
    
    sudo cp src-tauri/target/release/redragon-streamdeck /usr/local/bin/
    sudo chmod +x /usr/local/bin/redragon-streamdeck
    
    # Crear .desktop
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
    
    print_success "Aplicación compilada e instalada"
}

# Resumen
show_summary() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║              ¡Instalación Completada!                        ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Ejecuta: redragon-streamdeck"
    echo ""
    if ! groups | grep -q input; then
        echo -e "${YELLOW}⚠ IMPORTANTE: Cierra sesión y vuelve a iniciar para que funcionen los hotkeys${NC}"
    fi
}

# Main
main() {
    print_header
    check_distro
    install_dependencies
    setup_ydotool
    setup_udev
    install_rpm
    show_summary
}

main "$@"
