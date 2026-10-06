# Meeting Pake AI — catatan untuk AI/developer

Sumber kebenaran: **`PRD.md`**. Baca §0 sebelum mengerjakan apa pun. Kerjakan satu langkah §19 per sesi.

## Versi ter-pin (diambil dari lockfile, 2026-10-06)

| Komponen | Versi |
|---|---|
| Rust toolchain | stable-x86_64-pc-windows-msvc, rustc 1.99.0 |
| tauri / tauri-build | 2.12.1 / 2.7.1 (fitur `tray-icon`, `image-png`) |
| tauri-plugin-opener / -notification / -single-instance | 2.7.0 / 2.5.1 / 2.5.2 |
| serde / serde_json | 1.0.229 / 1.0.151 |
| rusqlite (bundled) | 0.40.2 |
| thiserror / uuid (v4) / chrono | 2.0.21 / 1.27.0 / 0.4.45 |
| keyring (default v1 → Windows Credential Manager) | 4.2.0 |
| reqwest (default rustls + json, multipart) | 0.13.5 |
| tracing / tracing-appender / tracing-subscriber (env-filter) | 0.1.44 / 0.2.5 / 0.3.23 |
| Node / npm | 22.17.0 / 10.9.2 |
| @tauri-apps/api / cli | 2.12.1 / 2.12.1 |
| @sveltejs/kit / adapter-static / vite-plugin-svelte | 2.70.3 / 3.0.10 / 7.3.1 |
| svelte / svelte-check / typescript | 5.57.2 / 4.7.6 / 6.0.3 |
| tailwindcss / @tailwindcss/vite | 4.3.3 / 4.3.3 |
| vite | 8.3.3 |

Crate lain (§6.1) ditambahkan di langkahnya masing-masing; catat versinya di tabel ini saat ditambahkan.

## Perintah

- Dev: `npm run tauri dev`
- Cek frontend: `npm run check`
- Cek Rust (dari `src-tauri/`): `cargo check` dan `cargo clippy -- -D warnings`
- Build installer: `npm run tauri build` (NSIS, per-user)
- Di shell sesi lama mungkin perlu `export PATH="$HOME/.cargo/bin:$PATH"`.

## Pengecualian dari PRD (keputusan pemilik proyek, 2026-10-06)

1. **Tanpa test otomatis.** Pemilik menguji manual. `cargo test` dan unit test wajib di §0/§16 tidak ditulis. Gerbang per langkah: `cargo check` + `cargo clippy -- -D warnings` + `npm run check` hijau, lalu checklist uji manual.
2. **Fase 0 spike dilewati** sebagai proyek terpisah. Item §21 diverifikasi di langkah terkait (model Groq di langkah 3, `wasapi`/loopback di langkah 4–5) dan hasilnya dicatat di bagian "Hasil verifikasi" di bawah.
3. Proyek di-scaffold langsung di root folder ini (bukan subfolder `meeting-pake-ai/`).

## Keputusan atas hal yang tidak tertulis di PRD

| Topik | Keputusan |
|---|---|
| `test_api_key` dengan key salah (401) | Kembalikan error `INVALID_API_KEY`. `{ ok, missingModels }` hanya untuk key valid. |
| Antrean dijeda karena 401 | Flag di memori; saat start dihitung ulang dari adanya meeting `failed` + `error_code = 'INVALID_API_KEY'`. Tanpa perubahan skema. |
| Nada tes onboarding | Sinus 1 kHz, −12 dBFS, 3 detik, dibangkitkan di kode. |
| Ikon | Placeholder: app = lingkaran indigo; tray `icons/tray-idle.png` (abu-abu) / `icons/tray-recording.png` (merah). |
| Cek disk saat merekam | Tiap 30 detik. |
| Argumen `update_settings` | `{ patch: Partial<Settings> }`. |
| tracing-subscriber | Ditambahkan (tidak ada di §6.1) karena dibutuhkan untuk memasang subscriber tracing ke file. |
| `providers.json` | Key yang hilang diisi dari default secara rekursif; `llm_extra_body` diganti utuh (bukan digabung) agar parameter default bisa dihapus. |
| Setting teks kosong | `userDisplayName`/`consentMessage` kosong setelah trim → kembali ke default. |
| Default providers | Disematkan di binary (`include_str!` dari `resources/providers.default.json`) lalu ditulis ke app data jika belum ada. |
| reqwest TLS | Fitur `rustls-tls` di PRD §6.1 sudah tidak ada di reqwest 0.13; TLS default 0.13 = rustls (aws-lc-rs). Tetap memenuhi NFR "TLS (rustls)". |
| Modul `groq.rs` | Bagian bersama Groq (client, `ProviderError`, pemetaan HTTP→error, `list_models`) di `src-tauri/src/groq.rs`; dipakai `stt/groq.rs` & `llm/groq.rs`. |
| `get_onboarding_status.micPermission` | Sementara selalu `unknown` sampai langkah 8. |
| UI API key | Bagian API key di Pengaturan dibuat di langkah 3 (agar bisa diuji); key diuji dulu, disimpan hanya jika lolos. |
| Rute dinamis `meeting/[id]` | `prerender = false` (dilayani lewat fallback SPA `index.html`). |

## Hasil verifikasi §21

- 2026-10-06: `GET /openai/v1/models` dengan key tidak valid → HTTP 401 (dipetakan ke `INVALID_API_KEY`).
- #1 nama model: _menunggu uji dengan key asli pemilik_.

## Progres langkah §19

- [x] 1. Scaffold Tauri 2 + SvelteKit SPA + Tailwind 4, tray, single-instance, 2 jendela
- [x] 2. `error.rs`, `db/` + migrasi 001 + repo, `config/`
- [x] 3. `secrets.rs` + command API key + `test_api_key`
- [ ] 4. Capture mic + writer part + repair header
- [ ] 5. Loopback + timeline padding + pause/mute + level
- [ ] 6. Command rekam + widget recorder + popup consent
- [ ] 7. Device change + auto-stop + cek disk
- [ ] 8. Onboarding UI + izin mic + tes 5 detik
- [ ] 9. VAD + chunker + offset map
- [ ] 10. GroqStt + rate limiter + retry
- [ ] 11. Filter + merge + dedup
- [ ] 12. GroqLlm + prompts + parse + map-reduce
- [ ] 13. Worker + state machine + recovery + retensi
- [ ] 14. UI Beranda + Detail + Pengaturan
- [ ] 15. Uji end-to-end, ukur NFR, build NSIS
