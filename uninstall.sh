#!/bin/bash

#═══════════════════════════════════════════════════════════════════════════════
#  Redragon Stream Deck Linux - Desinstalador
#  Por Tecnodespegue
#═══════════════════════════════════════════════════════════════════════════════

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

SERVICE_NAME="redragon-streamdeck"

echo -e "${RED}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║         DESINSTALAR REDRAGON STREAM DECK                      ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

echo -e "${YELLOW}Esto eliminará:${NC}"
echo "  - Servicio systemd"
echo "  - Reglas udev"
echo ""
echo -e "${CYAN}NO se eliminará:${NC}"
echo "  - El directorio del proyecto"
echo "  - Tu configuración (config.json)"
echo "  - Tus iconos"
echo ""

read -p "¿Continuar con la desinstalación? (s/n): " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Ss]$ ]]; then
    echo "Desinstalación cancelada."
    exit 0
fi

# Detener y deshabilitar servicio
echo -e "\n${CYAN}▶ Deteniendo servicio...${NC}"
systemctl --user stop $SERVICE_NAME.service 2>/dev/null || true
systemctl --user disable $SERVICE_NAME.service 2>/dev/null || true

# Eliminar archivo de servicio
SERVICE_FILE="$HOME/.config/systemd/user/$SERVICE_NAME.service"
if [ -f "$SERVICE_FILE" ]; then
    rm "$SERVICE_FILE"
    systemctl --user daemon-reload
    echo -e "${GREEN}✓ Servicio eliminado${NC}"
fi

# Eliminar reglas udev
echo -e "\n${CYAN}▶ Eliminando reglas udev...${NC}"
UDEV_FILE="/etc/udev/rules.d/99-redragon-streamdeck.rules"
if [ -f "$UDEV_FILE" ]; then
    sudo rm "$UDEV_FILE"
    sudo udevadm control --reload-rules
    echo -e "${GREEN}✓ Reglas udev eliminadas${NC}"
fi

echo -e "\n${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              DESINSTALACIÓN COMPLETADA                        ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "Para eliminar completamente el proyecto, ejecuta:"
echo -e "${YELLOW}  rm -rf $(pwd)${NC}"
echo ""
