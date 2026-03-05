// Zana Notification System
// Provides toast notifications for user feedback

(function() {
  // Create notification container
  const container = document.createElement('div');
  container.id = 'Zana-notifications';
  container.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 10000;
    display: flex;
    flex-direction: column;
    gap: 10px;
    pointer-events: none;
  `;
  document.body.appendChild(container);

  // Notification types and their styles
  const types = {
    success: { bg: '#a6e3a1', color: '#1e1e2e', icon: '✓' },
    error: { bg: '#f38ba8', color: '#1e1e2e', icon: '✕' },
    warning: { bg: '#f9e2af', color: '#1e1e2e', icon: '⚠' },
    info: { bg: '#89b4fa', color: '#1e1e2e', icon: 'ℹ' }
  };

  // Show notification
  window.ZanaNotify = function(message, type = 'info', duration = 3000) {
    const style = types[type] || types.info;

    const notification = document.createElement('div');
    notification.style.cssText = `
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 12px 16px;
      background: ${style.bg};
      color: ${style.color};
      border-radius: 8px;
      font-family: -apple-system, BlinkMacSystemFont, sans-serif;
      font-size: 14px;
      box-shadow: 0 4px 12px rgba(0,0,0,0.3);
      pointer-events: auto;
      transform: translateX(120%);
      transition: transform 0.3s ease;
    `;

    notification.innerHTML = `
      <span style="font-size: 16px;">${style.icon}</span>
      <span>${message}</span>
    `;

    container.appendChild(notification);

    // Animate in
    requestAnimationFrame(() => {
      notification.style.transform = 'translateX(0)';
    });

    // Auto dismiss
    if (duration > 0) {
      setTimeout(() => {
        notification.style.transform = 'translateX(120%)';
        setTimeout(() => notification.remove(), 300);
      }, duration);
    }

    return notification;
  };

  // Convenience methods
  window.ZanaSuccess = (msg, duration) => window.ZanaNotify(msg, 'success', duration);
  window.ZanaError = (msg, duration) => window.ZanaNotify(msg, 'error', duration);
  window.ZanaWarning = (msg, duration) => window.ZanaNotify(msg, 'warning', duration);
  window.ZanaInfo = (msg, duration) => window.ZanaNotify(msg, 'info', duration);

  // Listen for Tauri events
  if (window.__TAURI__) {
    window.__TAURI__.event.listen('notification', (event) => {
      const { message, type, duration } = event.payload;
      window.ZanaNotify(message, type, duration);
    });

    window.__TAURI__.event.listen('error', (event) => {
      window.ZanaError(event.payload.message || 'An error occurred');
    });

    window.__TAURI__.event.listen('transcription-complete', (event) => {
      window.ZanaSuccess('Transcribed and pasted!', 2000);
    });

    window.__TAURI__.event.listen('recording-started', () => {
      window.ZanaInfo('Recording...', 1500);
    });
  }
})();
