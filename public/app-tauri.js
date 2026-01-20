// ============================================================================
// Redragon Stream Deck - Tauri Desktop App
// Por Tecnodespegue
// ============================================================================

// Wait for Tauri API to be available
let invoke;
let dialogOpen;

let config = null;
let currentButtonId = null;
let editingPageIndex = null;
let selectedIconPath = null;
let presetCommands = [];

// ============================================================================
// Initialization
// ============================================================================

document.addEventListener('DOMContentLoaded', async () => {
  // Initialize Tauri API
  if (window.__TAURI__ && window.__TAURI__.core) {
    invoke = window.__TAURI__.core.invoke;
    console.log('Tauri API initialized successfully');
  } else {
    console.error('Tauri API not available!');
    document.getElementById('status-text').textContent = 'Error: Tauri API';
    return;
  }

  // Initialize dialog API if available
  if (window.__TAURI__ && window.__TAURI__.dialog) {
    dialogOpen = window.__TAURI__.dialog.open;
  }

  try {
    await loadConfig();
    await loadPresetCommands();
    // Auto-connect on startup
    await autoConnect();
    startButtonListener();
  } catch (e) {
    console.error('Initialization error:', e);
  }
});

// ============================================================================
// Preset Commands
// ============================================================================

async function loadPresetCommands() {
  try {
    presetCommands = await invoke('get_preset_commands');
    populatePresetDropdown();
  } catch (e) {
    console.error('Error loading preset commands:', e);
  }
}

function populatePresetDropdown() {
  const select = document.getElementById('preset-commands');
  if (!select) return;

  select.innerHTML = '<option value="">-- Comandos rápidos --</option>';

  // Group presets by category
  const categories = {
    'Multimedia': presetCommands.filter(p => ['Vol +', 'Vol -', 'Mute', 'Play/Pause', 'Next', 'Prev'].includes(p[0])),
    'Aplicaciones': presetCommands.filter(p => ['Firefox', 'Chrome', 'Terminal', 'Files', 'VS Code', 'Discord', 'Spotify', 'Steam', 'OBS'].includes(p[0])),
    'URLs': presetCommands.filter(p => ['YouTube', 'Twitch', 'GitHub', 'Twitter/X', 'ChatGPT', 'Claude'].includes(p[0])),
    'Hotkeys': presetCommands.filter(p => ['Copiar', 'Pegar', 'Cortar', 'Deshacer', 'Rehacer', 'Guardar', 'Buscar', 'Seleccionar todo', 'Cerrar ventana', 'Cambiar ventana', 'Pantalla completa', 'Emoji picker'].includes(p[0])),
    'Texto': presetCommands.filter(p => ['Email', 'Saludo', 'Firma'].includes(p[0])),
    'Multi-acción': presetCommands.filter(p => ['Abrir+Escribir', 'Copy+Paste'].includes(p[0])),
    'Fecha/Hora': presetCommands.filter(p => ['Reloj', 'Reloj+Seg', 'Fecha', 'Fecha completa', 'Día semana'].includes(p[0])),
    'Info Sistema': presetCommands.filter(p => ['CPU %', 'RAM %', 'Temp CPU'].includes(p[0])),
    'Timers': presetCommands.filter(p => p[0].startsWith('Timer ')),
    'Workspaces': presetCommands.filter(p => p[0].startsWith('WS ')),
    'Sistema': presetCommands.filter(p => ['Screenshot', 'Lock', 'Suspend'].includes(p[0])),
    'Navegación': presetCommands.filter(p => ['>> Next', '<< Prev', 'Home'].includes(p[0])),
  };

  for (const [category, items] of Object.entries(categories)) {
    if (items.length === 0) continue;
    const optgroup = document.createElement('optgroup');
    optgroup.label = category;
    for (const [label, command, description] of items) {
      const option = document.createElement('option');
      option.value = JSON.stringify({ label, command });
      option.textContent = `${label} - ${description}`;
      optgroup.appendChild(option);
    }
    select.appendChild(optgroup);
  }
}

function applyPreset(selectElement) {
  if (!selectElement.value) return;

  try {
    const { label, command } = JSON.parse(selectElement.value);
    document.getElementById('edit-label').value = label;
    document.getElementById('edit-command').value = command;
    selectElement.value = ''; // Reset dropdown
  } catch (e) {
    console.error('Error applying preset:', e);
  }
}

// Test command without saving
async function testCommand() {
  const command = document.getElementById('edit-command').value.trim();
  if (!command) {
    showToast('Ingresa un comando para probar');
    return;
  }

  // Don't test page navigation commands
  if (command === '__NEXT_PAGE__' || command === '__PREV_PAGE__' || command.startsWith('__PAGE_')) {
    showToast('Los comandos de navegación solo funcionan desde el dispositivo');
    return;
  }

  try {
    await invoke('run_command', { command });
    showToast('Comando ejecutado');
  } catch (e) {
    console.error('Error testing command:', e);
    showToast('Error al ejecutar comando');
  }
}

async function autoConnect() {
  try {
    // Check if udev rules are set up
    const rulesExist = await invoke('check_udev_rules');
    if (!rulesExist) {
      // Show notification and try to setup rules
      showToast('Configurando permisos USB...');
      try {
        await invoke('setup_udev_rules');
        showToast('Permisos USB configurados. Reconectando...');
        // Wait a moment for udev to apply
        await new Promise(resolve => setTimeout(resolve, 2000));
      } catch (e) {
        console.error('Error setting up udev rules:', e);
      }
    }

    // Try to connect to the device automatically
    const success = await invoke('connect_device');
    if (success) {
      try {
        await invoke('refresh_device');
      } catch (e) {
        console.log('Device refresh warning:', e);
      }
      showToast('Stream Deck conectado');
    } else {
      showToast('Conecta el Stream Deck y haz clic en Reconectar');
    }
    await checkStatus();
  } catch (e) {
    console.error('Auto-connect error:', e);
    await checkStatus();
  }
}

// ============================================================================
// Config Management
// ============================================================================

async function loadConfig() {
  try {
    config = await invoke('get_config');
    renderPageTabs();
    renderButtons();
    document.getElementById('brightness').value = config.brightness;
    document.getElementById('brightness-value').textContent = config.brightness;
  } catch (e) {
    console.error('Error loading config:', e);
  }
}

// ============================================================================
// Page Rendering
// ============================================================================

function renderPageTabs() {
  const container = document.getElementById('pages-tabs');
  container.innerHTML = '';

  config.pages.forEach((page, index) => {
    const tab = document.createElement('button');
    tab.className = `page-tab ${index === config.currentPage ? 'active' : ''}`;
    tab.innerHTML = `${page.name} <span class="edit-icon" onclick="event.stopPropagation(); editPageName(${index})">✎</span>`;
    tab.onclick = () => switchPage(index);
    container.appendChild(tab);
  });
}

function renderButtons() {
  const page = config.pages[config.currentPage];
  if (!page) return;

  for (const [id, btn] of Object.entries(page.buttons)) {
    const el = document.querySelector(`.button[data-id="${id}"]`);
    if (!el) continue;

    el.style.backgroundColor = btn.color || '#1a1a2e';
    el.innerHTML = '';

    if (btn.icon) {
      // Load icon as base64 data URL
      loadButtonIcon(el, btn.icon, btn.label);
    } else {
      el.style.backgroundImage = 'none';
      el.classList.remove('has-icon');
      el.textContent = btn.label || '';
    }
  }
}

// Helper function to load button icon as base64
async function loadButtonIcon(el, iconFilename, label) {
  try {
    const dataUrl = await invoke('get_icon_data', { filename: iconFilename });
    el.style.backgroundImage = `url('${dataUrl}')`;
    el.classList.add('has-icon');
    if (label) {
      const labelEl = document.createElement('span');
      labelEl.className = 'button-label';
      labelEl.textContent = label;
      el.appendChild(labelEl);
    }
  } catch (e) {
    console.error('Error loading icon:', iconFilename, e);
    el.style.backgroundImage = 'none';
    el.classList.remove('has-icon');
    el.textContent = label || '';
  }
}

// ============================================================================
// Page Navigation
// ============================================================================

async function switchPage(index) {
  try {
    await invoke('set_page', { index });
    config.currentPage = index;
    renderPageTabs();
    renderButtons();

    // Load the page on the device
    try {
      await invoke('load_current_page');
    } catch (e) {
      console.log('Could not update device:', e);
    }
  } catch (e) {
    console.error('Error switching page:', e);
  }
}

// ============================================================================
// Page Management
// ============================================================================

function addPage() {
  document.getElementById('new-page-name').value = '';
  document.getElementById('new-page-modal').classList.add('active');
  setTimeout(() => document.getElementById('new-page-name').focus(), 100);
}

function closeNewPageModal() {
  document.getElementById('new-page-modal').classList.remove('active');
}

async function createNewPage() {
  const name = document.getElementById('new-page-name').value.trim();
  if (!name) {
    document.getElementById('new-page-name').focus();
    return;
  }

  try {
    await invoke('add_page', { name });
    await loadConfig();
    closeNewPageModal();
    showToast(`Página "${name}" creada`);
  } catch (e) {
    console.error('Error adding page:', e);
  }
}

function showToast(message) {
  let toast = document.getElementById('toast');
  if (!toast) {
    toast = document.createElement('div');
    toast.id = 'toast';
    toast.className = 'toast';
    document.body.appendChild(toast);
  }
  toast.textContent = message;
  toast.classList.add('show');
  setTimeout(() => toast.classList.remove('show'), 3000);
}

function editPageName(index) {
  editingPageIndex = index;
  document.getElementById('page-name').value = config.pages[index].name;
  document.getElementById('page-modal').classList.add('active');
}

function closePageModal() {
  document.getElementById('page-modal').classList.remove('active');
  editingPageIndex = null;
}

async function savePageName() {
  if (editingPageIndex === null) return;

  const name = document.getElementById('page-name').value;
  try {
    await invoke('update_page_name', { index: editingPageIndex, name });
    config.pages[editingPageIndex].name = name;
    renderPageTabs();
    closePageModal();
  } catch (e) {
    console.error('Error saving page name:', e);
  }
}

function deletePage() {
  if (editingPageIndex === null) return;
  if (config.pages.length <= 1) {
    showToast('No puedes eliminar la única página');
    return;
  }

  const pageName = config.pages[editingPageIndex].name;
  document.getElementById('confirm-message').textContent = `¿Eliminar la página "${pageName}"? Esta acción no se puede deshacer.`;
  window.pendingAction = 'deletePage';
  document.getElementById('confirm-modal').classList.add('active');
}

async function clearPageButtons() {
  if (editingPageIndex === null) return;

  const pageName = config.pages[editingPageIndex].name;
  document.getElementById('confirm-message').textContent = `¿Limpiar todos los botones de la página "${pageName}"? Los botones volverán a su estado inicial.`;
  window.pendingAction = 'clearPage';
  document.getElementById('confirm-modal').classList.add('active');
}

async function executeClearPage() {
  if (editingPageIndex === null) return;

  try {
    await invoke('clear_page_buttons', { pageIndex: editingPageIndex });
    await loadConfig();
    closeConfirmModal();
    closePageModal();
    showToast('Botones limpiados');
  } catch (e) {
    console.error('Error clearing page:', e);
    showToast('Error al limpiar página');
  }
}

function closeConfirmModal() {
  document.getElementById('confirm-modal').classList.remove('active');
}

async function confirmDelete() {
  const action = window.pendingAction;
  window.pendingAction = null;

  // Handle different actions
  if (action === 'reset') {
    await executeReset();
    return;
  }

  if (action === 'clearPage') {
    await executeClearPage();
    return;
  }

  // Default: delete page
  if (editingPageIndex === null) return;

  const pageName = config.pages[editingPageIndex].name;
  try {
    await invoke('delete_page', { index: editingPageIndex });
    await loadConfig();
    closeConfirmModal();
    closePageModal();
    showToast(`Página "${pageName}" eliminada`);
  } catch (e) {
    console.error('Error deleting page:', e);
  }
}

// ============================================================================
// Device Status
// ============================================================================

async function checkStatus() {
  try {
    const status = await invoke('get_status');
    const indicator = document.getElementById('status-indicator');
    const text = document.getElementById('status-text');

    if (status.connected) {
      indicator.classList.add('connected');
      text.textContent = 'Conectado';
    } else {
      indicator.classList.remove('connected');
      text.textContent = 'Desconectado';
    }
  } catch (e) {
    console.error('Error checking status:', e);
  }
}

async function reconnect() {
  const btn = document.getElementById('reconnect-btn');
  btn.textContent = 'Conectando...';
  btn.disabled = true;

  try {
    const success = await invoke('connect_device');
    if (success) {
      await invoke('refresh_device');
      await checkStatus();
      await loadConfig();
    }
  } catch (e) {
    console.error('Error reconnecting:', e);
  }

  btn.innerHTML = '<span class="btn-icon">⟳</span> Reconectar';
  btn.disabled = false;
}

// ============================================================================
// Brightness
// ============================================================================

async function setBrightness(value) {
  document.getElementById('brightness-value').textContent = value;
  config.brightness = parseInt(value);

  try {
    await invoke('set_brightness_level', { brightness: parseInt(value) });
  } catch (e) {
    console.error('Error setting brightness:', e);
  }
}

// ============================================================================
// Button Editing
// ============================================================================

function editButton(id) {
  currentButtonId = id;
  selectedIconPath = null;
  const page = config.pages[config.currentPage];
  const btn = page.buttons[id] || { label: '', command: '', color: '#1a1a2e', icon: '' };

  document.getElementById('modal-btn-id').textContent = id;
  document.getElementById('edit-label').value = btn.label || '';
  document.getElementById('edit-command').value = btn.command || '';
  document.getElementById('edit-color').value = btn.color || '#1a1a2e';
  document.getElementById('edit-icon-path').value = '';

  const preview = document.getElementById('icon-preview');
  if (btn.icon) {
    // Load icon as base64 data URL
    invoke('get_icon_data', { filename: btn.icon })
      .then(dataUrl => {
        preview.style.backgroundImage = `url('${dataUrl}')`;
        preview.classList.add('has-icon');
      })
      .catch(e => {
        console.error('Error loading icon preview:', e);
        preview.style.backgroundImage = 'none';
        preview.classList.remove('has-icon');
      });
  } else {
    preview.style.backgroundImage = 'none';
    preview.classList.remove('has-icon');
  }

  // Reset preset dropdown
  const presetSelect = document.getElementById('preset-commands');
  if (presetSelect) presetSelect.value = '';

  document.getElementById('modal').classList.add('active');
}

// ============================================================================
// Icon Management
// ============================================================================

async function browseIcon() {
  try {
    // Use Tauri dialog to open file picker
    if (dialogOpen) {
      const selected = await dialogOpen({
        multiple: false,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp']
        }]
      });

      if (selected) {
        selectedIconPath = selected;
        // Show preview
        const preview = document.getElementById('icon-preview');
        // For preview, we need to convert to a data URL or use asset protocol
        preview.style.backgroundImage = `url(asset://localhost/${encodeURIComponent(selected)})`;
        preview.classList.add('has-icon');
        document.getElementById('edit-icon-path').value = selected;
        showToast('Imagen seleccionada');
      }
    } else {
      showToast('Selector de archivos no disponible');
    }
  } catch (e) {
    console.error('Error browsing icon:', e);
    showToast('Error al seleccionar imagen');
  }
}

// ============================================================================
// Reset Configuration
// ============================================================================

function confirmReset() {
  document.getElementById('confirm-message').textContent = '¿Borrar TODA la configuración y empezar de cero? Esta acción eliminará todas las páginas, botones e iconos.';
  document.getElementById('confirm-modal').classList.add('active');
  // Temporarily change confirmDelete to resetConfig
  window.pendingAction = 'reset';
}

async function executeReset() {
  try {
    await invoke('reset_config');
    await loadConfig();
    showToast('Configuración reiniciada');
    closeConfirmModal();
  } catch (e) {
    console.error('Error resetting config:', e);
    showToast('Error al reiniciar configuración');
  }
}

function closeModal() {
  document.getElementById('modal').classList.remove('active');
  currentButtonId = null;
}

function previewIcon(input) {
  if (input.files && input.files[0]) {
    const reader = new FileReader();
    reader.onload = (e) => {
      const preview = document.getElementById('icon-preview');
      preview.style.backgroundImage = `url(${e.target.result})`;
      preview.classList.add('has-icon');
    };
    reader.readAsDataURL(input.files[0]);
  }
}

async function removeIcon() {
  if (!currentButtonId) return;

  const pageIndex = config.currentPage;
  const preview = document.getElementById('icon-preview');
  preview.style.backgroundImage = 'none';
  preview.classList.remove('has-icon');
  document.getElementById('edit-icon').value = '';

  config.pages[pageIndex].buttons[currentButtonId].icon = '';
  renderButtons();
}

async function saveButton() {
  if (!currentButtonId) return;

  const pageIndex = config.currentPage;
  const label = document.getElementById('edit-label').value;
  const command = document.getElementById('edit-command').value;
  const color = document.getElementById('edit-color').value;
  const iconPath = document.getElementById('edit-icon-path').value;

  let icon = config.pages[pageIndex].buttons[currentButtonId]?.icon || '';

  // Handle icon from file picker
  if (selectedIconPath && iconPath) {
    try {
      // Generate a unique name for the icon
      const iconName = `btn_p${pageIndex}_b${currentButtonId}_${Date.now()}.png`;
      // Save the icon using Tauri backend
      icon = await invoke('save_icon', {
        sourcePath: selectedIconPath,
        iconName: iconName
      });
      showToast('Icono guardado');
    } catch (e) {
      console.error('Error saving icon:', e);
      showToast('Error al guardar icono');
    }
  }

  const buttonConfig = {
    label,
    command,
    color,
    icon
  };

  try {
    await invoke('update_button', {
      pageIndex,
      buttonId: currentButtonId.toString(),
      buttonConfig
    });

    config.pages[pageIndex].buttons[currentButtonId] = buttonConfig;
    renderButtons();
    closeModal();
    showToast('Botón guardado');

    // Reload the current page on the device to show the new button
    try {
      await invoke('load_current_page');
    } catch (e) {
      console.log('Could not update device:', e);
    }
  } catch (e) {
    console.error('Error saving button:', e);
    showToast('Error al guardar botón');
  }
}

// ============================================================================
// Button Listener (for physical device)
// ============================================================================

async function startButtonListener() {
  // In Tauri, we would set up a background task to listen for button presses
  // For now, just periodically check status
  setInterval(async () => {
    await checkStatus();
  }, 5000);
}

// ============================================================================
// Modal Event Handlers
// ============================================================================

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    closeModal();
    closePageModal();
    closeNewPageModal();
    closeConfirmModal();
  }
  if (e.key === 'Enter' && document.getElementById('new-page-modal').classList.contains('active')) {
    createNewPage();
  }
});

document.getElementById('modal').addEventListener('click', (e) => {
  if (e.target.id === 'modal') closeModal();
});

document.getElementById('page-modal').addEventListener('click', (e) => {
  if (e.target.id === 'page-modal') closePageModal();
});

document.getElementById('new-page-modal').addEventListener('click', (e) => {
  if (e.target.id === 'new-page-modal') closeNewPageModal();
});

document.getElementById('confirm-modal').addEventListener('click', (e) => {
  if (e.target.id === 'confirm-modal') closeConfirmModal();
});
