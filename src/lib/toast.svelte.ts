// Toast sederhana (state global dengan runes).

export type ToastKind = "info" | "success" | "error";
export type Toast = { id: number; text: string; kind: ToastKind };

export const toasts = $state<Toast[]>([]);
let nextId = 1;

export function showToast(text: string, kind: ToastKind = "info", ms = 4000) {
  const t = { id: nextId++, text, kind };
  toasts.push(t);
  setTimeout(() => dismissToast(t.id), ms);
}

export function dismissToast(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}
