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
  if (button) windowAction(button.dataset.window);
});

document.addEventListener('keydown', (event) => {
  const mod = event.metaKey || event.ctrlKey;
  const key = event.key.toLowerCase();
  if (mod && key === 'w') { event.preventDefault(); windowAction('close'); }
  else if (mod && key === 'm') { event.preventDefault(); windowAction('minimize'); }
  else if (key === 'escape' && document.body.hasAttribute('data-escape-closes')
    && !event.target.closest('input, textarea, select')) { windowAction('close'); }
});

document.addEventListener('DOMContentLoaded', () => {
  document.querySelectorAll('[data-window="minimize"]').forEach((b) => { b.title = 'Minimize (Command-M)'; });
  document.querySelectorAll('[data-window="close"]').forEach((b) => { b.title = 'Close (Command-W)'; });
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
