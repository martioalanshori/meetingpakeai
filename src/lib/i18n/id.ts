// SEMUA teks UI ada di sini (PRD §14, §15). Komponen tidak boleh berisi teks UI langsung.
import type { ErrorCode, MeetingStatus } from "../types";

export const id = {
  appName: "Meeting Pake AI",

  home: {
    startRecording: "● Mulai rekam",
    stopRecording: "■ Stop rekam",
    settings: "Pengaturan",
    empty: "Belum ada meeting. Klik Mulai rekam saat meeting dimulai.",
  },

  status: {
    recording: "Merekam…",
    queued: "Menunggu antrean",
    preprocessing: "Menyiapkan audio",
    transcribing: "Memproses transkrip",
    merging: "Menyusun transkrip",
    summarizing: "Membuat ringkasan",
    done: "Selesai",
    waiting_quota: "Menunggu kuota Groq",
    waiting_network: "Menunggu koneksi internet",
    failed: "Gagal",
    interrupted: "Rekaman terputus",
  } satisfies Record<MeetingStatus, string>,

  recorder: {
    paused: "Dijeda",
  },

  placeholder: {
    comingSoon: "Halaman ini dibangun di langkah berikutnya.",
    onboarding: "Onboarding",
    meeting: "Detail meeting",
    settings: "Pengaturan",
    back: "← Kembali ke Beranda",
  },

  errors: {
    NO_API_KEY: "API key Groq belum diatur. Buka Pengaturan untuk menambahkannya.",
    INVALID_API_KEY: "API key Groq tidak valid atau sudah dicabut. Perbarui di Pengaturan.",
    NETWORK: "Tidak bisa terhubung ke Groq. Periksa koneksi internet.",
    RATE_LIMITED: "Batas kecepatan Groq tercapai. Proses akan dilanjutkan otomatis.",
    QUOTA_EXHAUSTED: "Kuota harian Groq habis. Proses dilanjutkan otomatis besok.",
    MIC_PERMISSION_DENIED: "Akses mikrofon diblokir Windows. Izinkan di Pengaturan Privasi.",
    NO_INPUT_DEVICE: "Mikrofon tidak ditemukan.",
    NO_OUTPUT_DEVICE: "Perangkat audio output tidak ditemukan.",
    ALREADY_RECORDING: "Rekaman lain sedang berjalan.",
    NOT_RECORDING: "Tidak ada rekaman yang sedang berjalan.",
    DISK_FULL: "Ruang disk tidak cukup (minimal 1 GB).",
    NOT_FOUND: "Meeting tidak ditemukan.",
    INVALID_STATE: "Aksi ini tidak bisa dilakukan pada status meeting saat ini.",
    AUDIO_NOT_AVAILABLE: "Audio meeting ini sudah dihapus.",
    LLM_INVALID_OUTPUT: "Gagal membuat ringkasan. Coba buat ulang ringkasan.",
    INTERNAL: "Terjadi kesalahan. Detail tersimpan di log.",
  } satisfies Record<ErrorCode, string>,
};
