import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import '98.css';

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

type EventPayload<T> = {
  success: boolean;
  data: { type: string; data: T };
  error: string | null;
};

let queue: TrackInfo[] = [];
let currentTrack: TrackInfo | null = null;

function renderQueue() {
  trackList!.innerHTML = '';

  if (queue.length === 0) {
    pauseButton!.disabled = true;
    resumeButton!.disabled = true;
    nextButton!.disabled = true;
    prevButton!.disabled = true;
    trackProgress!.disabled = true;
    volumeSlider!.disabled = true;
    trackLooped!.disabled = true;

    trackPosition!.textContent = '0:00';
    trackDuration!.textContent = '0:00';
    trackProgress!.value = '0';
  } else {
    pauseButton!.disabled = false;
    resumeButton!.disabled = false;
    nextButton!.disabled = false;
    prevButton!.disabled = false;
    trackProgress!.disabled = false;
    volumeSlider!.disabled = false;
    trackLooped!.disabled = false;

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

      console.log(track.index, currentTrack);

      if (track.index === currentTrack?.index) {
        tr.classList.add('playing');
      }

      trackList!.appendChild(tr);
    });
  }
}

function addDOMEventListeners() {
  addQueueButton!.addEventListener('click', async () => {
    const path = await open({
      multiple: true,
      filters: [
        {
          name: 'Audio File',
          extensions: ['mp3', 'flac']
        }
      ]
    });
    if (path) {
      await invoke('add_queue', { filePaths: path });
    }
  });

  clearQueueButton!.addEventListener('click', () => {
    invoke('clear_queue');
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
  // Queue
  listen<EventPayload<TrackInfo[]>>('queue', (event) => {
    console.log(event);

    if (event.payload.success) {
      let newQueue = event.payload.data.data;

      if (newQueue.length === 0) {
        currentTrack = null;
      }

      queue = newQueue;

      renderQueue();
    }
  });

  // Play
  listen<EventPayload<{ index: number; track: TrackInfo }>>('play', (event) => {
    console.log(event);

    if (event.payload.success) {
      currentTrack = event.payload.data.data.track;

      renderQueue();

      pauseButton!.disabled = false;
      resumeButton!.disabled = true;
    }
  });

  // Status
  listen<EventPayload<String>>('status', (event) => {
    console.log(event);

    if (event.payload.success) {
      pauseButton!.disabled = true;
      resumeButton!.disabled = false;
    }
  });

  // Position
  listen<EventPayload<number>>('position', (event) => {
    console.log(event);

    if (event.payload.success) {
      trackProgress!.value = event.payload.data.data.toString();
      trackPosition!.textContent = formatTime(event.payload.data.data);
    }
  });

  // Looped
  listen<EventPayload<boolean>>('looped', (event) => {
    console.log(event);

    if (event.payload.success) {
      trackLooped!.checked = event.payload.data.data;
    }
  });

  // Volume
  listen<EventPayload<number>>('volume', (event) => {
    console.log(event);

    if (event.payload.success) {
      volumeSlider!.valueAsNumber = event.payload.data.data;
    }
  });

  listen<TrackProgress>('track-progress', (event) => {
    // console.log('Track Progress:', event.payload);
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
  renderQueue();
});
