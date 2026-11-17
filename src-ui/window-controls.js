// Minimize and close buttons for undecorated windows ([data-window] attribute),
// plus the shortcuts a native title bar would give: Command-W, Command-M, and
// Escape on windows that opt in with <body data-escape-closes>.
function currentWindow() {
  return window.__TAURI__?.window?.getCurrentWindow?.();
}

function windowAction(action) {
  const win = currentWindow();
  if (!win) return action === 'close' ? window.close() : undefined;
  return action === 'minimize' ? win.minimize() : win.close();
}

document.addEventListener('click', (event) => {
  const button = event.target.closest('[data-window]');
  if (button && !button.disabled) windowAction(button.dataset.window);
});

document.addEventListener('keydown', (event) => {
  const mod = event.metaKey || event.ctrlKey;
  const key = event.key.toLowerCase();
  if (mod && key === 'w') { event.preventDefault(); windowAction('close'); }
  else if (mod && key === 'm') { event.preventDefault(); windowAction('minimize'); }
  else if (key === 'escape' && document.body.hasAttribute('data-escape-closes')
    && !event.target.closest('input, textarea, select')) { windowAction('close'); }
});

// Native-looking buttons per platform: macOS traffic lights on the left
// (close, minimize, zoom), Windows caption buttons on the right
// (minimize, maximize, close). Zoom and maximize stay disabled because these
// windows are fixed-size.
const isMac = /Mac/i.test(navigator.platform || navigator.userAgent);
const ICONS = {
  close: '<path d="M2 2l6 6M8 2l-6 6"/>',
  minimize: '<path d="M2 5h6"/>',
  zoom: '<path d="M5 2v6M2 5h6"/>',
  maximize: '<rect x="2" y="2" width="6" height="6"/>',
};
const ORDER = isMac ? ['close', 'minimize', 'zoom'] : ['minimize', 'maximize', 'close'];
const LABELS = isMac
  ? { close: 'Close (Command-W)', minimize: 'Minimize (Command-M)', zoom: 'Zoom' }
  : { close: 'Close (Ctrl+W)', minimize: 'Minimize (Ctrl+M)', maximize: 'Maximize' };

document.addEventListener('DOMContentLoaded', () => {
  document.body.classList.add(isMac ? 'platform-mac' : 'platform-windows');
  document.querySelectorAll('.window-controls').forEach((group) => {
    group.classList.add(isMac ? 'mac' : 'windows');
    group.innerHTML = ORDER.map((action) => {
      const fixed = action === 'zoom' || action === 'maximize';
      return `<button type="button" data-window="${action}" aria-label="${LABELS[action].replace(/ \(.*/, '')}" title="${LABELS[action]}"${fixed ? ' disabled' : ''}><svg viewBox="0 0 10 10" aria-hidden="true">${ICONS[action]}</svg></button>`;
    }).join('');
  });
});

// Drag the window from anywhere inside a [data-tauri-drag-region] header,
// including its text and logo, but not from buttons or fields inside it.
document.addEventListener('mousedown', (event) => {
  if (event.button !== 0) return;
  if (!event.target.closest('[data-tauri-drag-region]')) return;
  if (event.target.closest('button, a, input, select, textarea, [data-window]')) return;
  const win = currentWindow();
  if (!win) return;
  event.preventDefault();
  if (event.detail === 2) {
    return;
  }
  win.startDragging();
});
