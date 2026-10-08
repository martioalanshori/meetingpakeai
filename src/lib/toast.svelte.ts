// Toast sederhana (state global dengan runes).

export type ToastKind = "info" | "success" | "error";
export type Toast = { id: number; text: string; kind: ToastKind };

export const toasts = $state<Toast[]>([]);
let nextId = 1;

/** Maksimal toast yang tampil bersamaan; yang paling lama ditutup. */
const MAX_TOASTS = 3;

/** Error bertahan lebih lama (8 dtk) agar sempat dibaca. */
export function showToast(text: string, kind: ToastKind = "info", ms?: number) {
  const t = { id: nextId++, text, kind };
  toasts.push(t);
  while (toasts.length > MAX_TOASTS) toasts.shift();
  setTimeout(() => dismissToast(t.id), ms ?? (kind === "error" ? 8000 : 4000));
}

export function dismissToast(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}
