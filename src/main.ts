import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { formatTime } from './util';
import { initWindow } from './window';
import '98.css';

type TrackInfo = {
  index: number;
  title: string;
  album: string;
  artist: string;
  duration: number;
};

type EventPayload<T> = {
  success: boolean;
  data: { type: string; data: T };
  error: string | null;
};

const elements = {
  trackDuration: document.getElementById('track-duration') as HTMLParagraphElement,
  trackPosition: document.getElementById('track-position') as HTMLParagraphElement,
  trackProgress: document.getElementById('track-timeline') as HTMLInputElement,
  trackLooped: document.getElementById('track-looped') as HTMLInputElement,
  volumeSlider: document.getElementById('volume-slider') as HTMLInputElement,
  trackList: document.getElementById('track-list') as HTMLTableSectionElement,
  addQueueButton: document.getElementById('add-queue-btn') as HTMLButtonElement,
  clearQueueButton: document.getElementById('clear-queue-btn') as HTMLButtonElement,
  resumeButton: document.getElementById('resume-btn') as HTMLButtonElement,
  pauseButton: document.getElementById('pause-btn') as HTMLButtonElement,
  nextButton: document.getElementById('next-btn') as HTMLButtonElement,
  prevButton: document.getElementById('prev-btn') as HTMLButtonElement
};

let queue: TrackInfo[] = [];
let currentTrack: TrackInfo | null = null;
let isPlaying = false;

function updateUIState() {
  const isQueueEmpty = queue.length === 0;

  elements.pauseButton.disabled = isQueueEmpty || !currentTrack || !isPlaying;
  elements.resumeButton.disabled = isQueueEmpty || isPlaying;
  elements.nextButton.disabled = isQueueEmpty || !currentTrack || isPlaying;
  elements.prevButton.disabled = isQueueEmpty || !currentTrack || isPlaying;
  elements.trackProgress.disabled = isQueueEmpty || !currentTrack;

  if (isQueueEmpty) {
    elements.trackPosition.textContent = '0:00';
    elements.trackDuration.textContent = '0:00';
    elements.trackProgress.value = '0';
  }
}

function renderQueue() {
  elements.trackList.innerHTML = '';

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

    tr.addEventListener('dblclick', () => invoke('play', { index: track.index }));
    if (track.index === currentTrack?.index) tr.classList.add('playing');

    elements.trackList.appendChild(tr);
  });
}

function addDOMEventListeners() {
  elements.addQueueButton.addEventListener('click', async () => {
    const path = await open({
      multiple: true,
      filters: [{ name: 'Audio File', extensions: ['mp3', 'flac'] }]
    });
    if (path) await invoke('add_queue', { filePaths: path });
  });

  elements.clearQueueButton.addEventListener('click', () => invoke('clear_queue'));
  elements.resumeButton.addEventListener('click', () => invoke('resume'));
  elements.pauseButton.addEventListener('click', () => invoke('pause'));
  elements.prevButton.addEventListener('click', () => invoke('prev'));
  elements.nextButton.addEventListener('click', () => invoke('next'));

  elements.trackLooped.addEventListener('input', () =>
    invoke('set_looped', { looped: elements.trackLooped.checked })
  );
  elements.volumeSlider.addEventListener('input', () =>
    invoke('set_volume', { volume: elements.volumeSlider.valueAsNumber / 100 })
  );
  elements.trackProgress.addEventListener('change', () =>
    invoke('set_position', { position: elements.trackProgress.valueAsNumber })
  );
}

function addTauriListeners() {
  listen<EventPayload<TrackInfo[]>>('queue', (event) => {
    if (event.payload.success) {
      queue = event.payload.data.data;
      currentTrack = queue.length ? currentTrack : null;
      renderQueue();
      updateUIState();
    }
  });

  listen<EventPayload<{ index: number; track: TrackInfo }>>('play', (event) => {
    if (event.payload.success) {
      currentTrack = event.payload.data.data.track;
      isPlaying = true;
      renderQueue();
      updateUIState();
    }
  });

  listen<EventPayload<string>>('status', (event) => {
    if (event.payload.success) {
      isPlaying = false;
      updateUIState();
    }
  });

  listen<EventPayload<number>>('position', (event) => {
    if (event.payload.success) {
      const duration = currentTrack?.duration || event.payload.data.data;
      elements.trackProgress.value = event.payload.data.data.toString();
      elements.trackProgress.max = duration.toString();
      elements.trackPosition.textContent = formatTime(event.payload.data.data);
      elements.trackDuration.textContent = formatTime(duration);
    }
  });

  listen<EventPayload<boolean>>('looped', (event) => {
    if (event.payload.success) elements.trackLooped.checked = event.payload.data.data;
  });

  // listen<EventPayload<number>>('volume', (event) => {
  //   if (event.payload.success) elements.volumeSlider.valueAsNumber = event.payload.data.data * 100;
  // });
}

window.addEventListener('DOMContentLoaded', () => {
  initWindow();
  addDOMEventListeners();
  addTauriListeners();
  renderQueue();
  updateUIState();
});
