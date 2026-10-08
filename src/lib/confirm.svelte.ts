// Dialog konfirmasi bersama (menggantikan confirm() bawaan browser). Satu ConfirmHost di layout.

export type ConfirmOptions = {
  title: string;
  message?: string;
  confirmText: string;
  cancelText?: string;
  /** Aksi merusak (hapus): tombol konfirmasi merah. */
  danger?: boolean;
};

export const confirmState = $state<{ opts: ConfirmOptions | null }>({ opts: null });

let pending: ((ok: boolean) => void) | null = null;

/** Tampilkan dialog dan tunggu pilihan pengguna (`true` = konfirmasi). */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  pending?.(false);
  confirmState.opts = opts;
  return new Promise((resolve) => (pending = resolve));
}

export function settleConfirm(ok: boolean) {
  const resolve = pending;
  pending = null;
  confirmState.opts = null;
  resolve?.(ok);
}
