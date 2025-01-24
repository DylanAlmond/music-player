import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import '98.css';

import './window';
import { formatTime } from './util';
import { initWindow } from './window';

let trackDuration: HTMLParagraphElement | null;
let trackPosition: HTMLParagraphElement | null;
let trackProgress: HTMLInputElement | null;
let trackLooped: HTMLInputElement | null;
let volumeSlider: HTMLInputElement | null;
let trackList: HTMLTableSectionElement | null;

let addQueueButton: HTMLButtonElement | null;
let clearQueueButton: HTMLButtonElement | null;
let resumeButton: HTMLButtonElement | null;
let pauseButton: HTMLButtonElement | null;
let nextButton: HTMLButtonElement | null;
let prevButton: HTMLButtonElement | null;

type TrackInfo = {
  index: number;
  title: string;
  album: string;
  artist: string;
  duration: number;
};

type TrackProgress = {
  position: number;
  duration: number;
};

let queue: TrackInfo[] = [];

function renderQueue() {
  trackList!.innerHTML = '';
  queue.forEach((track) => {
    const tr = document.createElement('tr');
    tr.dataset.index = track.index.toString();

    tr.innerHTML = `
      <td>${track.title}</td>
      <td>${track.artist}</td>
      <td>${track.album}</td>
      <td>${formatTime(track.duration)}</td>
    `;

    tr.addEventListener('click', () => {
      document.querySelectorAll('.highlighted').forEach((e) => e.classList.remove('highlighted'));
      tr.classList.add('highlighted');
    });

    tr.addEventListener('dblclick', () => {
      invoke('play', { index: track.index });
    });

    trackList!.appendChild(tr);
  });
}

function addDOMEventListeners() {
  addQueueButton!.addEventListener('click', async () => {
    const path = await open({ multiple: true });
    if (path) {
      const res: TrackInfo[] = await invoke('add_queue', { filePaths: path });
      queue = res;
      renderQueue();
    }
  });

  clearQueueButton!.addEventListener('click', () => {
    invoke('clear_queue');
    queue = [];
    renderQueue();
  });
  resumeButton!.addEventListener('click', () => invoke('resume'));
  pauseButton!.addEventListener('click', () => invoke('pause'));
  prevButton!.addEventListener('click', () => invoke('prev'));
  nextButton!.addEventListener('click', () => invoke('next'));

  trackLooped!.addEventListener('input', () =>
    invoke('set_looped', { looped: trackLooped!.checked })
  );
  volumeSlider!.addEventListener('input', () =>
    invoke('set_volume', { volume: volumeSlider!.valueAsNumber / 100 })
  );
  trackProgress!.addEventListener('change', () =>
    invoke('set_position', { position: trackProgress!.valueAsNumber })
  );
}

function addTauriListeners() {
  listen<TrackInfo>('track-change', (event) => {
    document.querySelectorAll('.playing').forEach((e) => e.classList.remove('playing'));
    const elem = document.querySelector(`[data-index="${event.payload.index}"]`) as HTMLLIElement;
    if (elem) elem.classList.add('playing');
  });

  listen<TrackProgress>('track-progress', (event) => {
    console.log('Track Progress:', event.payload);
    trackProgress!.value = event.payload.position.toString();
    trackProgress!.max = event.payload.duration.toString();
    trackPosition!.textContent = formatTime(event.payload.position);
    trackDuration!.textContent = formatTime(event.payload.duration);
  });
}

window.addEventListener('DOMContentLoaded', () => {
  trackDuration = document.getElementById('track-duration') as HTMLParagraphElement;
  trackPosition = document.getElementById('track-position') as HTMLParagraphElement;
  trackProgress = document.getElementById('track-timeline') as HTMLInputElement;
  trackLooped = document.getElementById('track-looped') as HTMLInputElement;
  volumeSlider = document.getElementById('volume-slider') as HTMLInputElement;
  trackList = document.getElementById('track-list') as HTMLTableSectionElement;

  addQueueButton = document.getElementById('add-queue-btn') as HTMLButtonElement;
  clearQueueButton = document.getElementById('clear-queue-btn') as HTMLButtonElement;
  resumeButton = document.getElementById('resume-btn') as HTMLButtonElement;
  pauseButton = document.getElementById('pause-btn') as HTMLButtonElement;
  nextButton = document.getElementById('next-btn') as HTMLButtonElement;
  prevButton = document.getElementById('prev-btn') as HTMLButtonElement;

  initWindow();
  addDOMEventListeners();
  addTauriListeners();
});
