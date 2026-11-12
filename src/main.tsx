import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from '@/app/App';
import '@/styles/global.css';

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root element #root not found');
}

// Files dropped on the window are read only where a screen asks for them (the import dialog);
// anywhere else a drop does nothing, so the webview never opens a dropped file itself.
for (const type of ['dragover', 'drop'] as const) {
  window.addEventListener(type, (event) => {
    event.preventDefault();
  });
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
