// Preferensi tampilan per pengguna (disimpan di localStorage, aman bila penyimpanan tidak tersedia).

const RAIL_KEY = "rail-hidden";

function readBool(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

export const ui = $state({ railHidden: typeof window !== "undefined" && readBool(RAIL_KEY) });

/** Sembunyikan / tampilkan sidebar kiri (Ctrl+B). */
export function toggleRail(hidden = !ui.railHidden) {
  ui.railHidden = hidden;
  try {
    localStorage.setItem(RAIL_KEY, hidden ? "1" : "0");
  } catch {
    /* penyimpanan tidak tersedia: tetap berlaku untuk sesi ini */
  }
}
