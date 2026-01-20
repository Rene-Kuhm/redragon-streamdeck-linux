#!/bin/bash

# =============================================================================
# Redragon Stream Deck Manager - Desinstalador
# =============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}"
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║        Redragon Stream Deck Manager - Desinstalador          ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

read -p "¿Estás seguro de que deseas desinstalar? [y/N] " -n 1 -r
echo

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelado."
    exit 0
fi

echo -e "${GREEN}[+]${NC} Deteniendo servicios..."
systemctl --user stop redragon-streamdeck.service 2>/dev/null || true
systemctl --user disable redragon-streamdeck.service 2>/dev/null || true

echo -e "${GREEN}[+]${NC} Eliminando binario..."
sudo rm -f /usr/local/bin/redragon-streamdeck

echo -e "${GREEN}[+]${NC} Eliminando entrada de escritorio..."
rm -f ~/.local/share/applications/redragon-streamdeck.desktop

echo -e "${GREEN}[+]${NC} Eliminando servicio systemd..."
rm -f ~/.config/systemd/user/redragon-streamdeck.service
systemctl --user daemon-reload 2>/dev/null || true

read -p "¿Eliminar configuración y datos? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${GREEN}[+]${NC} Eliminando configuración..."
    rm -rf ~/.local/share/redragon-streamdeck
    rm -rf ~/.config/redragon-streamdeck
fi

read -p "¿Eliminar reglas udev? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${GREEN}[+]${NC} Eliminando reglas udev..."
    sudo rm -f /etc/udev/rules.d/99-redragon-streamdeck.rules
    sudo udevadm control --reload-rules
fi

echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              Desinstalación Completada                       ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
