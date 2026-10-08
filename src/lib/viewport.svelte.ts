// Ukuran jendela untuk tata letak yang tidak bisa diatur CSS saja (mis. Beranda dua panel).
// Sama dengan breakpoint Tailwind `xl` (80rem = 1280 px).
export const WIDE_QUERY = "(min-width: 80rem)";

const media = typeof window !== "undefined" ? window.matchMedia(WIDE_QUERY) : null;

export const viewport = $state({ wide: media?.matches ?? false });

media?.addEventListener("change", (e) => (viewport.wide = e.matches));
