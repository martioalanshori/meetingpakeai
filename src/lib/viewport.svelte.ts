// Breakpoint tata letak yang tidak bisa diatur CSS saja. Angka yang sama dipakai di kelas Tailwind:
// - dua panel Beranda (daftar + notulen): ≥ 1100 px (`min-[68.75rem]:`)
// - rel kiri penuh (ikon + label): ≥ 1280 px (`xl:`); di bawahnya rel ringkas agar panel detail cukup lebar.
export const SPLIT_QUERY = "(min-width: 68.75rem)";

const media = typeof window !== "undefined" ? window.matchMedia(SPLIT_QUERY) : null;

export const viewport = $state({ wide: media?.matches ?? false });

media?.addEventListener("change", (e) => (viewport.wide = e.matches));

/** Tab terakhir per meeting: dipertahankan saat tata letak berganti (panel kanan ↔ halaman sendiri). */
export const detailTab = $state({ meetingId: "", tab: "" });
