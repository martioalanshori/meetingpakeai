// Preferensi tampilan per pengguna (disimpan di localStorage, aman bila penyimpanan tidak tersedia).

const RAIL_KEY = "rail-collapsed";
/** Sama dengan breakpoint `xl` Tailwind: lebar default rel penuh. */
const WIDE_QUERY = "(min-width: 80rem)";

function readChoice(): boolean | null {
  try {
    const v = localStorage.getItem(RAIL_KEY);
    return v === "1" ? true : v === "0" ? false : null;
  } catch {
    return null;
  }
}

const media = typeof window !== "undefined" ? window.matchMedia(WIDE_QUERY) : null;
const wide = $state({ on: media?.matches ?? true });
media?.addEventListener("change", (e) => (wide.on = e.matches));

/** `collapsed`: pilihan pengguna (true = rel ikon, false = rel penuh); null = ikut lebar jendela. */
export const ui = $state({ collapsed: typeof window !== "undefined" ? readChoice() : null });

/** Rel ringkas (ikon saja) atau penuh (ikon + label). */
export function railCollapsed(): boolean {
  return ui.collapsed ?? !wide.on;
}

/** Ringkas / lebarkan sidebar kiri (tombol di rel atau Ctrl+B); pilihan diingat. */
export function toggleRail(collapsed = !railCollapsed()) {
  ui.collapsed = collapsed;
  try {
    localStorage.setItem(RAIL_KEY, collapsed ? "1" : "0");
  } catch {
    /* penyimpanan tidak tersedia: tetap berlaku untuk sesi ini */
  }
}
