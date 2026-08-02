let config = null;
let currentButtonId = null;
let editingPageIndex = null;

document.addEventListener('DOMContentLoaded', async () => {
  await loadConfig();
  await checkStatus();
});

async function loadConfig() {
  try {
    const res = await fetch('/api/config');
    config = await res.json();
    renderPageTabs();
    renderButtons();
    document.getElementById('brightness').value = config.brightness;
    document.getElementById('brightness-value').textContent = config.brightness;
  } catch (e) {
    console.error('Error loading config:', e);
  }
}

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
      el.style.backgroundImage = `url(/icons/${btn.icon}?t=${Date.now()})`;
      el.classList.add('has-icon');
      if (btn.label) {
        const labelEl = document.createElement('span');
        labelEl.className = 'button-label';
        labelEl.textContent = btn.label;
        el.appendChild(labelEl);
      }
    } else {
      el.style.backgroundImage = 'none';
      el.classList.remove('has-icon');
      el.textContent = btn.label || '';
    }
  }
}

async function switchPage(index) {
  try {
    await fetch(`/api/page/${index}`);
    config.currentPage = index;
    renderPageTabs();
    renderButtons();
  } catch (e) {
    console.error('Error switching page:', e);
  }
}

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
    const res = await fetch('/api/page', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name })
    });
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
    await fetch(`/api/page/${editingPageIndex}/name`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name })
    });
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
  document.getElementById('confirm-modal').classList.add('active');
}

function closeConfirmModal() {
  document.getElementById('confirm-modal').classList.remove('active');
}

async function confirmDelete() {
  if (editingPageIndex === null) return;

  const pageName = config.pages[editingPageIndex].name;
  try {
    await fetch(`/api/page/${editingPageIndex}`, { method: 'DELETE' });
    await loadConfig();
    closeConfirmModal();
    closePageModal();
    showToast(`Página "${pageName}" eliminada`);
  } catch (e) {
    console.error('Error deleting page:', e);
  }
}

async function checkStatus() {
  try {
    const res = await fetch('/api/status');
    const { connected } = await res.json();
    const indicator = document.getElementById('status-indicator');
    const text = document.getElementById('status-text');

    if (connected) {
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
    const res = await fetch('/api/reconnect', { method: 'POST' });
    const { success } = await res.json();
    if (success) {
      await checkStatus();
      await loadConfig();
    }
  } catch (e) {
    console.error('Error reconnecting:', e);
  }

  btn.textContent = 'Reconectar';
  btn.disabled = false;
}

async function setBrightness(value) {
  document.getElementById('brightness-value').textContent = value;
  config.brightness = parseInt(value);

  try {
    await fetch('/api/brightness', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ brightness: parseInt(value) })
    });
  } catch (e) {
    console.error('Error setting brightness:', e);
  }
}

function editButton(id) {
  currentButtonId = id;
  const page = config.pages[config.currentPage];
  const btn = page.buttons[id];

  document.getElementById('modal-btn-id').textContent = id;
  document.getElementById('edit-label').value = btn.label || '';
  document.getElementById('edit-command').value = btn.command || '';
  document.getElementById('edit-color').value = btn.color || '#1a1a2e';

  const preview = document.getElementById('icon-preview');
  if (btn.icon) {
    preview.style.backgroundImage = `url(/icons/${btn.icon}?t=${Date.now()})`;
    preview.classList.add('has-icon');
  } else {
    preview.style.backgroundImage = 'none';
    preview.classList.remove('has-icon');
  }

  document.getElementById('edit-icon').value = '';
  document.getElementById('modal').classList.add('active');
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
  try {
    await fetch(`/api/page/${pageIndex}/button/${currentButtonId}/icon`, { method: 'DELETE' });

    const preview = document.getElementById('icon-preview');
    preview.style.backgroundImage = 'none';
    preview.classList.remove('has-icon');
    document.getElementById('edit-icon').value = '';

    config.pages[pageIndex].buttons[currentButtonId].icon = '';
    renderButtons();
  } catch (e) {
    console.error('Error removing icon:', e);
  }
}

async function saveButton() {
  if (!currentButtonId) return;

  const pageIndex = config.currentPage;
  const label = document.getElementById('edit-label').value;
  const command = document.getElementById('edit-command').value;
  const color = document.getElementById('edit-color').value;
  const iconInput = document.getElementById('edit-icon');

  // Upload icon if selected
  if (iconInput.files && iconInput.files[0]) {
    const formData = new FormData();
    formData.append('icon', iconInput.files[0]);

    try {
      const res = await fetch(`/api/page/${pageIndex}/button/${currentButtonId}/icon`, {
        method: 'POST',
        body: formData
      });
      const { filename } = await res.json();
      config.pages[pageIndex].buttons[currentButtonId].icon = filename;
    } catch (e) {
      console.error('Error uploading icon:', e);
    }
  }

  // Save button config
  const buttonConfig = {
    label,
    command,
    color,
    icon: config.pages[pageIndex].buttons[currentButtonId].icon || ''
  };

  try {
    await fetch(`/api/page/${pageIndex}/button/${currentButtonId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buttonConfig)
    });

    config.pages[pageIndex].buttons[currentButtonId] = buttonConfig;
    renderButtons();
    closeModal();
  } catch (e) {
    console.error('Error saving button:', e);
  }
}

// Close modals on escape key
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    closeModal();
    closePageModal();
    closeNewPageModal();
    closeConfirmModal();
  }
  // Enter key to submit new page
  if (e.key === 'Enter' && document.getElementById('new-page-modal').classList.contains('active')) {
    createNewPage();
  }
});

// Close modals on outside click
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
