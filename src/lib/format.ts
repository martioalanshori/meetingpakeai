// Format Indonesia (PRD §14): tanggal "6 Okt 2026 14.00", durasi "1 j 02 m" / "12 m 05 d", transkrip "HH:MM:SS".

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"];

const pad = (n: number) => String(n).padStart(2, "0");

/** epoch ms (UTC) → "6 Okt 2026 14.00" di zona waktu lokal. */
export function formatDateTime(ms: number): string {
  const d = new Date(ms);
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()} ${pad(d.getHours())}.${pad(d.getMinutes())}`;
}

/** epoch ms → "14.00". */
export function formatTime(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}.${pad(d.getMinutes())}`;
}

/** durasi ms → "1 j 02 m" (≥ 1 jam) atau "12 m 05 d". */
export function formatDuration(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return h > 0 ? `${h} j ${pad(m)} m` : `${m} m ${pad(s)} d`;
}

/** posisi timeline ms → "HH:MM:SS". */
export function formatTimestamp(ms: number): string {
  const total = Math.floor(ms / 1000);
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total % 3600) / 60))}:${pad(total % 60)}`;
}
