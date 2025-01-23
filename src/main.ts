import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

let trackName: HTMLSpanElement | null;
let trackDuration: HTMLParagraphElement | null;
let trackPosition: HTMLParagraphElement | null;
let trackProgress: HTMLInputElement | null;
let trackLooped: HTMLInputElement | null;
let volumeSlider: HTMLInputElement | null;

let trackList: HTMLOListElement | null;

let playQueueButton: HTMLButtonElement | null;
let resumeButton: HTMLButtonElement | null;
let pauseButton: HTMLButtonElement | null;
let nextButton: HTMLButtonElement | null;
let prevButton: HTMLButtonElement | null;

type TrackInfo = {
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
    const li = document.createElement('li');
    li.textContent = `${track.title} - ${track.album} - ${track.artist} - ${track.duration}`;
    li.dataset.title = track.title;

    trackList!.appendChild(li);
  });
}

window.addEventListener('DOMContentLoaded', () => {
  trackName = document.getElementById('track-name') as HTMLSpanElement;
  trackDuration = document.getElementById('track-duration') as HTMLParagraphElement;
  trackPosition = document.getElementById('track-position') as HTMLParagraphElement;
  trackProgress = document.getElementById('track-timeline') as HTMLInputElement;
  trackLooped = document.getElementById('track-looped') as HTMLInputElement;
  volumeSlider = document.getElementById('volume-slider') as HTMLInputElement;

  trackList = document.getElementById('track-list') as HTMLOListElement;

  playQueueButton = document.getElementById('play-queue-btn') as HTMLButtonElement;
  resumeButton = document.getElementById('resume-btn') as HTMLButtonElement;
  pauseButton = document.getElementById('pause-btn') as HTMLButtonElement;
  nextButton = document.getElementById('next-btn') as HTMLButtonElement;
  prevButton = document.getElementById('prev-btn') as HTMLButtonElement;

  playQueueButton.addEventListener('click', async () => {
    const path = await open({
      multiple: true
    });

    if (path) {
      const res: TrackInfo[] = await invoke('play_queue', { filePaths: path });
      queue = res;
      renderQueue();
    }
  });

  // Resume audio playback
  resumeButton.addEventListener('click', async () => {
    await invoke('resume');
  });

  // Pause audio playback
  pauseButton.addEventListener('click', async () => {
    await invoke('pause');
  });

  // Play the previous audio file
  prevButton.addEventListener('click', () => {
    invoke('prev');
  });

  // Play the next audio file
  nextButton.addEventListener('click', () => {
    invoke('next');
  });

  trackLooped.addEventListener('input', () => {
    invoke('set_looped', { looped: trackLooped!.checked });
  });

  volumeSlider.addEventListener('input', () => {
    let v = volumeSlider!.valueAsNumber / 100;
    console.log(volumeSlider!.valueAsNumber, v);

    invoke('set_volume', { volume: v });
  });

  listen<TrackInfo>('track-change', (event) => {
    const playing = document.querySelectorAll('.playing');
    playing.forEach((e) => {
      e.classList.remove('playing');
    });

    console.log(`Track changed:`, event.payload);
    trackName!.textContent = `${event.payload.title} | ${event.payload.album} | ${
      event.payload.artist
    } | ${formatTime(event.payload.duration)}`;

    const elem =
      (document.querySelector(`[data-title="${event.payload.title}"]`) as HTMLLIElement) || null;
    if (elem) {
      elem.classList.add('playing');
    }
  });

  function formatTime(position: number): string {
    const minutes = Math.floor(position / 60);
    const seconds = Math.floor(position % 60);
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  }

  trackProgress.addEventListener('change', () => {
    console.log(`Setting Position: ${trackProgress!.valueAsNumber}`);
    invoke('set_position', { position: trackProgress!.valueAsNumber });
  });

  listen<TrackProgress>('track-progress', (event) => {
    console.log(`Track Progress:`, event.payload);

    trackProgress!.value = event.payload.position.toString();
    trackProgress!.max = event.payload.duration.toString();

    trackPosition!.textContent = formatTime(event.payload.position);
    trackDuration!.textContent = formatTime(event.payload.duration);
  });
});
