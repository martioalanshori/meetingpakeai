// Jawaban "Tanya semua meeting" terakhir: tetap ada saat pengguna kembali ke Beranda (tidak disimpan permanen).
import type { AskAllResult } from "./types";

export const lastAsk = $state<{ question: string; answer: AskAllResult | null }>({ question: "", answer: null });
