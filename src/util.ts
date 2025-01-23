export function formatTime(position: number): string {
  const minutes = Math.floor(position / 60);
  const seconds = Math.floor(position % 60);
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}
