#!/bin/bash

#═══════════════════════════════════════════════════════════════════════════════
#  Redragon Stream Deck Linux - Instalador Automático
#  Por Tecnodespegue
#═══════════════════════════════════════════════════════════════════════════════

set -e

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Directorio del proyecto
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_NAME="redragon-streamdeck"

#───────────────────────────────────────────────────────────────────────────────
print_banner() {
    echo -e "${PURPLE}"
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║                                                               ║"
    echo "║   ██████╗ ███████╗██████╗ ██████╗  █████╗  ██████╗  ██████╗  ║"
    echo "║   ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔════╝ ██╔═══██╗ ║"
    echo "║   ██████╔╝█████╗  ██║  ██║██████╔╝███████║██║  ███╗██║   ██║ ║"
    echo "║   ██╔══██╗██╔══╝  ██║  ██║██╔══██╗██╔══██║██║   ██║██║   ██║ ║"
    echo "║   ██║  ██║███████╗██████╔╝██║  ██║██║  ██║╚██████╔╝╚██████╔╝ ║"
    echo "║   ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝  ╚═════╝  ║"
    echo "║                                                               ║"
    echo "║              STREAM DECK LINUX - INSTALADOR                   ║"
    echo "║                   Por Tecnodespegue                           ║"
    echo "║                                                               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_step() {
    echo -e "\n${CYAN}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

#───────────────────────────────────────────────────────────────────────────────
check_root() {
    if [ "$EUID" -eq 0 ]; then
        print_error "No ejecutes este script como root. Se pedirá sudo cuando sea necesario."
        exit 1
    fi
}

#───────────────────────────────────────────────────────────────────────────────
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO=$ID
    elif [ -f /etc/arch-release ]; then
        DISTRO="arch"
    elif [ -f /etc/debian_version ]; then
        DISTRO="debian"
    else
        DISTRO="unknown"
    fi
    echo $DISTRO
}

#───────────────────────────────────────────────────────────────────────────────
install_dependencies() {
    print_step "Instalando dependencias del sistema..."

    DISTRO=$(detect_distro)

    case $DISTRO in
        arch|cachyos|endeavouros|manjaro)
            print_step "Detectado: Arch Linux / $DISTRO"
            sudo pacman -S --needed --noconfirm nodejs npm imagemagick libusb
            ;;
        debian|ubuntu|linuxmint|pop)
            print_step "Detectado: Debian / Ubuntu"
            sudo apt update
            sudo apt install -y nodejs npm imagemagick libusb-1.0-0-dev libudev-dev
            ;;
        fedora)
            print_step "Detectado: Fedora"
            sudo dnf install -y nodejs npm ImageMagick libusb1-devel systemd-devel
            ;;
        opensuse*)
            print_step "Detectado: openSUSE"
            sudo zypper install -y nodejs npm ImageMagick libusb-1_0-devel
            ;;
        *)
            print_warning "Distribución no reconocida: $DISTRO"
            print_warning "Instala manualmente: nodejs, npm, imagemagick, libusb"
            read -p "¿Continuar de todos modos? (s/n): " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Ss]$ ]]; then
                exit 1
            fi
            ;;
    esac

    print_success "Dependencias del sistema instaladas"
}

#───────────────────────────────────────────────────────────────────────────────
setup_udev_rules() {
    print_step "Configurando reglas udev para acceso USB..."

    UDEV_RULE='SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", MODE="0666", TAG+="uaccess"'
    UDEV_FILE="/etc/udev/rules.d/99-redragon-streamdeck.rules"

    echo "$UDEV_RULE" | sudo tee $UDEV_FILE > /dev/null

    sudo udevadm control --reload-rules
    sudo udevadm trigger

    print_success "Reglas udev configuradas"
    print_warning "Si el Stream Deck está conectado, desconéctalo y vuélvelo a conectar"
}

#───────────────────────────────────────────────────────────────────────────────
install_npm_dependencies() {
    print_step "Instalando dependencias de Node.js..."

    cd "$SCRIPT_DIR"
    npm install

    print_success "Dependencias de Node.js instaladas"
}

#───────────────────────────────────────────────────────────────────────────────
setup_config() {
    print_step "Configurando archivo de configuración..."

    if [ ! -f "$SCRIPT_DIR/config.json" ]; then
        cp "$SCRIPT_DIR/config.example.json" "$SCRIPT_DIR/config.json"
        print_success "Archivo config.json creado"
    else
        print_warning "config.json ya existe, no se sobrescribirá"
    fi

    # Crear directorio de iconos si no existe
    mkdir -p "$SCRIPT_DIR/icons"
}

#───────────────────────────────────────────────────────────────────────────────
setup_systemd_service() {
    print_step "Configurando servicio systemd para auto-inicio..."

    SERVICE_DIR="$HOME/.config/systemd/user"
    SERVICE_FILE="$SERVICE_DIR/$SERVICE_NAME.service"

    mkdir -p "$SERVICE_DIR"

    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=Redragon Stream Deck Manager
After=graphical-session.target

[Service]
Type=simple
WorkingDirectory=$SCRIPT_DIR
ExecStart=/usr/bin/node $SCRIPT_DIR/node_modules/.bin/tsx src/server.ts
Restart=on-failure
RestartSec=5
Environment=NODE_ENV=production

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload
    systemctl --user enable $SERVICE_NAME.service

    print_success "Servicio systemd configurado y habilitado"
}

#───────────────────────────────────────────────────────────────────────────────
start_service() {
    print_step "Iniciando servicio..."

    systemctl --user start $SERVICE_NAME.service
    sleep 2

    if systemctl --user is-active --quiet $SERVICE_NAME.service; then
        print_success "Servicio iniciado correctamente"
    else
        print_error "Error al iniciar el servicio"
        echo "Revisa los logs con: journalctl --user -u $SERVICE_NAME.service -f"
        exit 1
    fi
}

#───────────────────────────────────────────────────────────────────────────────
print_final_message() {
    echo -e "\n${GREEN}"
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║                                                               ║"
    echo "║              ¡INSTALACIÓN COMPLETADA!                         ║"
    echo "║                                                               ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"

    echo -e "${CYAN}Accede a la interfaz web en:${NC}"
    echo -e "${YELLOW}    http://localhost:3000${NC}"
    echo ""
    echo -e "${CYAN}Comandos útiles:${NC}"
    echo -e "    ${YELLOW}Ver estado:${NC}     systemctl --user status $SERVICE_NAME"
    echo -e "    ${YELLOW}Ver logs:${NC}       journalctl --user -u $SERVICE_NAME -f"
    echo -e "    ${YELLOW}Reiniciar:${NC}      systemctl --user restart $SERVICE_NAME"
    echo -e "    ${YELLOW}Detener:${NC}        systemctl --user stop $SERVICE_NAME"
    echo ""
    echo -e "${PURPLE}¡Disfruta tu Stream Deck! - Tecnodespegue${NC}"
    echo ""
}

#───────────────────────────────────────────────────────────────────────────────
# MAIN
#───────────────────────────────────────────────────────────────────────────────

print_banner
check_root

echo -e "${YELLOW}Este script instalará y configurará el Redragon Stream Deck.${NC}"
echo -e "${YELLOW}Se requerirá tu contraseña de sudo para algunas operaciones.${NC}"
echo ""
read -p "¿Continuar con la instalación? (s/n): " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Ss]$ ]]; then
    echo "Instalación cancelada."
    exit 0
fi

install_dependencies
setup_udev_rules
install_npm_dependencies
setup_config
setup_systemd_service
start_service
print_final_message
