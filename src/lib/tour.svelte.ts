// Tur aplikasi interaktif: menyorot elemen asli (atribut `data-tour`) satu per satu, berpindah
// halaman bila perlu. Dimulai otomatis setelah onboarding; bisa diulang dari rel kiri dan
// Pengaturan → Bantuan. Teks di `id.guide.steps`.
import { id } from "$lib/i18n/id";

type StepKey = keyof typeof id.guide.steps;

export type TourStep = {
  key: StepKey;
  /** Nilai `data-tour` elemen yang disorot. */
  target: string;
  /** Halaman tempat elemen berada (path + query yang wajib cocok); null = tetap di halaman sekarang. */
  route: string | null;
  /** Elemen bisa tidak ada (mis. belum ada meeting) → langkah dilewati. */
  optional?: boolean;
  /** Klik elemen yang disorot = lanjut (mis. menu navigasi). */
  advanceOnClick?: boolean;
};

export const TOUR_STEPS: TourStep[] = [
  { key: "record", target: "record", route: "/" },
  { key: "ask", target: "home-ask", route: "/" },
  { key: "later", target: "home-later", route: "/", optional: true },
  { key: "navMeetings", target: "nav-meetings", route: "/", advanceOnClick: true },
  { key: "import", target: "meetings-import", route: "/meetings" },
  { key: "search", target: "meetings-search", route: "/meetings" },
  { key: "list", target: "meetings-list", route: "/meetings", optional: true },
  { key: "detailTabs", target: "detail-tabs", route: "/meetings", optional: true },
  { key: "navTasks", target: "nav-tasks", route: "/meetings", advanceOnClick: true },
  { key: "tasks", target: "tasks-title", route: "/tasks" },
  { key: "navSettings", target: "nav-settings", route: "/tasks", advanceOnClick: true },
  { key: "settingsTabs", target: "settings-tabs", route: "/settings?tab=recording" },
  { key: "glossary", target: "settings-glossary", route: "/settings?tab=recording" },
  { key: "shortcut", target: "settings-shortcut", route: "/settings?tab=recording", optional: true },
  { key: "tabAi", target: "settings-tab-ai", route: "/settings?tab=recording", advanceOnClick: true },
  { key: "ai", target: "settings-ai", route: "/settings?tab=ai" },
  { key: "guide", target: "guide", route: null },
];

export const tour = $state({ active: false, index: 0 });

export function startTour() {
  tour.index = 0;
  tour.active = true;
}

export function endTour() {
  tour.active = false;
}
