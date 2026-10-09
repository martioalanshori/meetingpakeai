// Format Indonesia (PRD §14): tanggal "6 Okt 2026 14.00", durasi "1 j 02 m" / "12 m 05 d", transkrip "HH:MM:SS".

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"];

const pad = (n: number) => String(n).padStart(2, "0");

/** epoch ms → "6 Okt 2026" di zona waktu lokal. */
export function formatDate(ms: number): string {
  const d = new Date(ms);
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()}`;
}

/** epoch ms (UTC) → "6 Okt 2026 14.00" di zona waktu lokal. */
export function formatDateTime(ms: number): string {
  return `${formatDate(ms)} ${formatTime(ms)}`;
}

/** epoch ms → "14.00". */
export function formatTime(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}.${pad(d.getMinutes())}`;
}

/** durasi ms → "1 jam 2 mnt", "32 mnt", atau "45 dtk" (detik hanya untuk < 1 menit). */
export function formatDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  if (total < 60) return `${total} dtk`;
  const minutes = Math.round(total / 60);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} mnt`;
  return m === 0 ? `${h} jam` : `${h} jam ${m} mnt`;
}

/** posisi timeline ms → "HH:MM:SS". */
export function formatTimestamp(ms: number): string {
  const total = Math.floor(ms / 1000);
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total % 3600) / 60))}:${pad(total % 60)}`;
}

const DAYS = ["Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu"];

/** Judul kelompok hari: "Hari ini", "Kemarin", "Senin, 5 Okt" (tahun ditulis jika bukan tahun ini). */
export function dayLabel(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const today = new Date(now);
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diffDays = Math.round((startOf(today) - startOf(d)) / 86_400_000);
  if (diffDays === 0) return "Hari ini";
  if (diffDays === 1) return "Kemarin";
  const year = d.getFullYear() === today.getFullYear() ? "" : ` ${d.getFullYear()}`;
  return `${DAYS[d.getDay()]}, ${d.getDate()} ${MONTHS[d.getMonth()]}${year}`;
}
