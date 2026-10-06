// SPA: Tauri tidak punya server Node, jadi SSR dimatikan dan semua halaman lewat fallback index.html.
export const ssr = false;
export const prerender = true;
