import { getCurrentWindow } from '@tauri-apps/api/window';
import { getName, getVersion } from '@tauri-apps/api/app';

export function initWindow() {
  const appWindow = getCurrentWindow();

  getName().then((v) => {
    const textElem = document.getElementById('title-bar-text') as HTMLDivElement | null;
    console.log(textElem);

    if (textElem) {
      textElem.innerHTML = v;
    }
  });

  getVersion().then((v) => {
    const textElem = document.getElementById('app-version') as HTMLSpanElement | null;
    if (textElem) {
      textElem.textContent = `v${v}`;
    }
  });

  document
    .getElementById('title-bar-minimize')
    ?.addEventListener('click', () => appWindow.minimize());
  document
    .getElementById('title-bar-maximize')
    ?.addEventListener('click', () => appWindow.toggleMaximize());
  document.getElementById('title-bar-close')?.addEventListener('click', () => appWindow.close());
}
