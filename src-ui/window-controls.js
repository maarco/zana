// Minimize and close buttons for undecorated windows ([data-window] attribute).
document.addEventListener('click', (event) => {
  const button = event.target.closest('[data-window]');
  if (!button) return;
  const win = window.__TAURI__?.window?.getCurrentWindow?.();
  if (!win) return window.close();
  if (button.dataset.window === 'minimize') win.minimize();
  else win.close();
});
