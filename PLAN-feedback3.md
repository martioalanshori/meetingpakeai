# Rencana eksekusi `feedback3.md`

**Aturan:**
- Satu langkah = satu commit.
- Tanpa pengujian oleh AI; pemilik menguji manual.
- Gerbang tiap langkah: `npm run check` + `cargo clippy --all-targets -- -D warnings`.
- Keputusan dicatat di `CLAUDE.md`.

| Langkah | Isi (ID `feedback3.md`) |
|---|---|
| 45 | Akurasi dasar: G1 aturan filter + rapikan pengulangan, G2 prompt kalimat, G7 padding VAD & normalisasi level, G8 alat ukur WER |
| 46 | Tidak ada meeting terlewat: A0.1 tawaran di widget, A0.2 rekam otomatis + hitung mundur, A0.3 pengingat ulang, A0.4 kenali Google Meet, A0.5 status deteksi, A2 judul dari jendela meeting |
| 47 | A1 Impor rekaman (mp3/m4a/mp4/wav/ogg/flac/webm) |
| 48 | B1 Transkrip sementara + Ringkas sejauh ini, F1 kartu "notulen siap" setelah Stop |
| 49 | B2 Catatan saya, B3 pengingat jeda, B4 judul di widget, F4 tandai momen sesudah meeting |
| 50 | C2 Intisari & pertanyaan terbuka, C4 instruksi buat ulang, C5 bahasa notulen |
| 51 | C1 Tanya meeting ini |
| 52 | C3 Perbaiki transkrip + ganti semua + glosarium (G10) |
| 53 | D1 Tanya semua meeting |
| 54 | D2 Tugas terstruktur: ubah PJ/tenggat, tambah manual, pengingat tenggat, .ics |
| 55 | D3 Meeting rutin + status tindak lanjut |
| 56 | Akurasi lanjutan: G3 konteks antar-potongan, G4 Akurasi tinggi, G5 bahasa campuran, G6 coba ulang bagian ragu; E1 indikator kualitas, E2 poin tanpa rujukan |
| 57 | Penyempurnaan: A4 menu tray, F2 tombol Bagikan, F3 notulen kosong, C6 gabung meeting, C7 hapus bagian, D4 label proyek, D5 ringkasan mingguan, G9 rapikan transkrip dengan AI |

**Ditunda (butuh akun/OAuth atau evaluasi perangkat):**
- D6 integrasi To Do/kalender (.ics di langkah 54 sebagai pengganti sementara).
- E3 transkripsi lokal whisper.cpp (preset Kustom tetap jalan sementara).
