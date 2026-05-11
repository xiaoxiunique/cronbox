function pad(value: number) {
  return String(value).padStart(2, "0");
}

export function formatDateTime(iso: string | null | undefined) {
  if (!iso) return "-";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return [
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`,
  ].join(" ");
}

export function formatDuration(ms: number | null | undefined) {
  if (ms == null || !Number.isFinite(ms)) return "-";
  if (ms < 1000) return `${Math.round(ms)} ms`;

  const seconds = ms / 1000;
  if (seconds < 10) return `${Number(seconds.toFixed(1))} s`;
  if (seconds < 60) return `${Math.round(seconds)} s`;

  const roundedSeconds = Math.round(seconds);
  const minutes = Math.floor(roundedSeconds / 60);
  const remainingSeconds = roundedSeconds % 60;
  if (minutes < 60) {
    return remainingSeconds ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
  }

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}
