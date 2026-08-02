// ============================================================================
// Redragon Stream Deck — capa de interfaz
// Por Tecnodespegue
//
// Todo lo que hay acá es presentación: la biblioteca de acciones, la
// paginación numérica, los paneles laterales y la selección de tecla.
// No habla con Tauri; se apoya en las funciones que ya expone app-tauri.js.
//
// Se carga DESPUÉS de app-tauri.js pero ANTES de que dispare DOMContentLoaded,
// así que los envoltorios de más abajo ya están puestos en el primer render.
// ============================================================================

// ── Biblioteca de acciones (columna derecha) ────────────────────────────────

// Mismo agrupamiento que usa el desplegable de comandos rápidos. Cada entrada
// es [etiqueta, prefijos...]; un preset cae en la primera categoría que lo
// reclame, y lo que no reclama nadie termina en "Otros" en vez de perderse.
const ACTION_CATEGORIES = [
  ['Multimedia',    '#c1272d', ['Vol +', 'Vol -', 'Mute', 'Play/Pause', 'Next', 'Prev']],
  ['Aplicaciones',  '#0078ff', ['Firefox', 'Chrome', 'Terminal', 'Files', 'VS Code', 'Discord', 'Spotify', 'Steam', 'OBS']],
  ['Páginas web',   '#8b5cf6', ['YouTube', 'Twitch', 'GitHub', 'Twitter/X', 'ChatGPT', 'Claude']],
  ['Atajos',        '#0ea5e9', ['Copiar', 'Pegar', 'Cortar', 'Deshacer', 'Rehacer', 'Guardar', 'Buscar', 'Seleccionar todo', 'Cerrar ventana', 'Cambiar ventana', 'Pantalla completa', 'Emoji picker']],
  ['Texto',         '#f59e0b', ['Email', 'Saludo', 'Firma']],
  ['Acción múltiple','#ec4899', ['Abrir+Escribir', 'Copy+Paste']],
  ['Fecha y hora',  '#14b8a6', ['Reloj', 'Reloj+Seg', 'Fecha', 'Fecha completa', 'Día semana']],
  ['Monitor del sistema', '#22c55e', ['CPU %', 'RAM %', 'Temp CPU']],
  ['Temporizadores','#a855f7', ['Timer ']],
  ['Escritorios',   '#3b82f6', ['WS ']],
  ['Sistema',       '#64748b', ['Screenshot', 'Lock', 'Suspend']],
  ['Navegación',    '#eab308', ['>> Next', '<< Prev', 'Home']],
];

function groupPresets(presets) {
  const groups = ACTION_CATEGORIES.map(([name, color]) => ({ name, color, items: [] }));
  const otros = { name: 'Otros', color: '#6b7280', items: [] };

  for (const preset of presets) {
    const label = preset[0];
    const index = ACTION_CATEGORIES.findIndex(([, , matchers]) =>
      matchers.some(m => (m.endsWith(' ') ? label.startsWith(m) : label === m))
    );
    (index === -1 ? otros : groups[index]).items.push(preset);
  }

  groups.push(otros);
  return groups.filter(g => g.items.length > 0);
}

function renderActionCategories() {
  const container = document.getElementById('action-categories');
  if (!container) return;

  container.innerHTML = '';

  const groups = groupPresets(presetCommands || []);
  if (groups.length === 0) {
    container.innerHTML = '<div class="library-empty">No hay acciones disponibles.</div>';
    return;
  }

  for (const group of groups) {
    const cat = document.createElement('div');
    cat.className = 'cat';

    const head = document.createElement('button');
    head.className = 'cat-head';
    head.innerHTML =
      '<svg class="cat-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">' +
      '<polyline points="9 18 15 12 9 6"/></svg>' +
      `<span class="cat-badge" style="background:${group.color}"></span>` +
      `<span class="cat-name">${escapeHtml(group.name)}</span>` +
      `<span class="cat-count">${group.items.length}</span>`;
    head.onclick = () => cat.classList.toggle('open');

    const items = document.createElement('div');
    items.className = 'cat-items';

    for (const [label, command, description] of group.items) {
      const row = document.createElement('button');
      row.className = 'action-row';
      row.dataset.search = `${label} ${description || ''}`.toLowerCase();
      row.innerHTML =
        `<span class="action-name">${escapeHtml(label)}</span>` +
        `<span class="action-desc">${escapeHtml(description || '')}</span>`;
      row.onclick = () => applyAction(label, command);
      items.appendChild(row);
    }

    cat.appendChild(head);
    cat.appendChild(items);
    container.appendChild(cat);
  }
}

// Aplica una acción de la biblioteca a la tecla seleccionada. Sin tecla
// seleccionada no hay dónde ponerla, así que se avisa en vez de no hacer nada.
function applyAction(label, command) {
  if (!currentButtonId) {
    showToast('Seleccione primero una tecla');
    return;
  }
  const labelInput = document.getElementById('edit-label');
  const commandInput = document.getElementById('edit-command');
  if (!labelInput.value.trim()) labelInput.value = label;
  commandInput.value = command;
  showToast(`Acción «${label}» lista — pulse Guardar`);
}

function filterActions(query) {
  const q = (query || '').trim().toLowerCase();
  const cats = document.querySelectorAll('#action-categories .cat');

  cats.forEach(cat => {
    let visible = 0;
    cat.querySelectorAll('.action-row').forEach(row => {
      const match = !q || row.dataset.search.includes(q);
      row.style.display = match ? '' : 'none';
      if (match) visible++;
    });
    cat.style.display = visible ? '' : 'none';
    // Con una búsqueda activa conviene ver los resultados sin tener que abrir
    // cada categoría a mano; al vaciarla se vuelve al estado plegado.
    if (q) cat.classList.add('open');
    else cat.classList.remove('open');
  });
}

// ── Paginación numérica bajo la grilla ──────────────────────────────────────

function renderPagePills() {
  const container = document.getElementById('page-pills');
  if (!container || !config) return;

  container.innerHTML = '';

  config.pages.forEach((page, index) => {
    const pill = document.createElement('button');
    pill.className = `page-pill ${index === config.currentPage ? 'active' : ''}`;
    pill.textContent = index + 1;
    pill.title = page.name;
    pill.onclick = () => switchPage(index);
    container.appendChild(pill);
  });

  const add = document.createElement('button');
  add.className = 'page-pill';
  add.textContent = '+';
  add.title = 'Nueva página';
  add.onclick = () => addPage();
  container.appendChild(add);
}

// ── Selección de tecla ──────────────────────────────────────────────────────

function highlightSelectedKey(id) {
  document.querySelectorAll('.button.selected').forEach(el => el.classList.remove('selected'));
  if (id === null || id === undefined) return;
  const el = document.querySelector(`.button[data-id="${id}"]`);
  if (el) el.classList.add('selected');
}

// Vacía la tecla abierta en el panel de propiedades y guarda.
function clearCurrentButton() {
  if (!currentButtonId) return;
  document.getElementById('edit-label').value = '';
  document.getElementById('edit-command').value = '';
  document.getElementById('edit-hotkey').value = '';
  removeIcon();
  saveButton();
}

// ── Páginas: acciones de la barra lateral ───────────────────────────────────

// deletePage() y clearPageButtons() trabajan sobre editingPageIndex, que sólo
// se completa al abrir el diálogo de edición. Desde la barra lateral se opera
// sobre la página actual, así que hay que fijarlo antes de llamarlas.
function deleteCurrentPage() {
  if (!config) return;
  editingPageIndex = config.currentPage;
  deletePage();
}

// ── Paneles laterales ───────────────────────────────────────────────────────

function togglePanel(id) {
  const panel = document.getElementById(id);
  if (!panel) return;
  const opening = !panel.classList.contains('open');
  document.querySelectorAll('.side-panel.open').forEach(p => p.classList.remove('open'));
  if (opening) panel.classList.add('open');
}

function toggleHelpPanel() { togglePanel('help-panel'); }

function toggleSettingsPanel() {
  togglePanel('settings-panel');
  // Al abrir se refresca el estado: comprobar contra GitHub cada vez que se
  // arranca la aplicación sería innecesario, pero al mirar los ajustes es
  // justo cuando interesa saber si hay algo nuevo.
  if (document.getElementById('settings-panel').classList.contains('open')) {
    refrescarEstadoDeVersion();
  }
}

// ── Versión instalada ───────────────────────────────────────────────────────

function pintarEstado(texto, clase) {
  const el = document.getElementById('update-status');
  if (!el) return;
  el.textContent = texto;
  el.className = `about-status ${clase}`;
}

// Lee la versión que este binario declara. La graba build.rs a partir de la
// etiqueta de git, así que avanza sola con cada release.
async function cargarVersionInstalada() {
  try {
    const [version, commit] = await invoke('get_current_version');
    const conPrefijo = `v${version}`;

    const enBarra = document.getElementById('titlebar-version');
    if (enBarra) enBarra.textContent = conPrefijo;

    const enPanel = document.getElementById('app-version');
    if (enPanel) enPanel.textContent = conPrefijo;

    const elCommit = document.getElementById('app-commit');
    if (elCommit) elCommit.textContent = commit || '—';
  } catch (e) {
    console.error('No se pudo leer la versión instalada:', e);
  }
}

async function refrescarEstadoDeVersion() {
  pintarEstado('Comprobando actualizaciones…', 'is-checking');
  try {
    const info = await invoke('check_for_updates');
    if (info.available) {
      pintarEstado(`Disponible ${info.latest_commit_short}`, 'is-outdated');
    } else {
      pintarEstado('Estás en la última versión', 'is-current');
    }
  } catch (e) {
    // Sin conexión, o GitHub limitando peticiones: no es un fallo de la
    // aplicación y no conviene presentarlo como tal.
    pintarEstado('No se pudo comprobar', 'is-error');
  }
}

// El botón de los ajustes: refresca la línea de estado y, si hay algo nuevo,
// deja que la lógica de siempre muestre el diálogo con el detalle.
async function buscarActualizaciones() {
  await refrescarEstadoDeVersion();
  checkForUpdates();
}

// ── Envoltorios sobre app-tauri.js ──────────────────────────────────────────
// Se envuelve en vez de editar las funciones originales para que la lógica de
// la app y la de la interfaz queden separadas.

(function wireUp() {
  const origRenderPageTabs = window.renderPageTabs;
  window.renderPageTabs = function () {
    origRenderPageTabs.apply(this, arguments);
    renderPagePills();
  };

  const origPopulatePresets = window.populatePresetDropdown;
  window.populatePresetDropdown = function () {
    origPopulatePresets.apply(this, arguments);
    renderActionCategories();
  };

  const origEditButton = window.editButton;
  window.editButton = function (id) {
    origEditButton.apply(this, arguments);
    highlightSelectedKey(id);
  };

  const origCloseModal = window.closeModal;
  window.closeModal = function () {
    origCloseModal.apply(this, arguments);
    highlightSelectedKey(null);
  };

  // Desde la barra lateral se limpia la página actual, no la que quedó abierta
  // en el diálogo.
  const origClearPageButtons = window.clearPageButtons;
  window.clearPageButtons = function () {
    if (editingPageIndex === null && config) editingPageIndex = config.currentPage;
    return origClearPageButtons.apply(this, arguments);
  };
})();

// Este oyente se registra después del de app-tauri.js, que asigna `invoke`
// antes de su primer `await`; para cuando corre esto, ya está disponible.
document.addEventListener('DOMContentLoaded', () => {
  cargarVersionInstalada();
});

// Cerrar diálogos y paneles con Escape.
document.addEventListener('keydown', e => {
  if (e.key !== 'Escape') return;
  const openPanel = document.querySelector('.side-panel.open');
  if (openPanel) { openPanel.classList.remove('open'); return; }
  const openDialog = document.querySelector('.modal-overlay.active');
  if (openDialog) openDialog.classList.remove('active');
});

// Clic fuera del diálogo para cerrarlo.
document.addEventListener('click', e => {
  if (e.target.classList && e.target.classList.contains('modal-overlay')) {
    e.target.classList.remove('active');
  }
});
