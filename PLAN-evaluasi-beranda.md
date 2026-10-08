# Rencana evaluasi: halaman Beranda

**Basis:** commit `6a72667`. File utama: `src/routes/+page.svelte`, data dari `src-tauri/src/commands/home.rs`.

**Lensa:** desain produk (frontend-design). Pertanyaannya bukan "apakah sudah jalan", tetapi:
- apakah Beranda ini terasa *milik* Meeting Pake AI,
- apakah Beranda membantu orang yang baru saja selesai meeting,
- apakah ada yang terlihat seperti template.

**Aturan:**
- Evaluasi dulu, lalu perbaikan sebagai langkah terpisah (satu commit per langkah).
- AI tidak menjalankan pengujian; tangkapan layar dibuat dengan data contoh (lihat §2).

---

## 1. Subjek, pengguna, tugas utama

| | |
|---|---|
| **Subjek** | Notetaker meeting desktop yang merekam *dua suara*: mikrofon pengguna dan audio komputer (peserta lain). Sistem desain "Dua suara": kertas `#EEF1EC`, lembar `#FFFFFF`, tinta `#1C1F26` / `#1E2433`, suara Saya `#0E6B6B`, suara peserta `#A3450F`, merah rekam `#D92D20`, Plus Jakarta Sans. |
| **Pengguna** | Pekerja kantor Indonesia yang meeting 3–8 kali sehari (Zoom/Teams/Meet), membuka aplikasi *setelah* meeting atau saat butuh mengingat sesuatu. |
| **Tugas utama Beranda** | Dalam 5 detik menjawab: (1) "apa yang dibahas/diputuskan?" → Tanya, (2) "apa yang harus saya kerjakan?" → Tugas mendesak, (3) "apakah meeting saya terekam?" → status siap. |

Tolok ukur keberhasilan:
- Hal pertama yang dilihat mata adalah kolom Tanya.
- Tidak ada blok yang bisa dihapus tanpa kehilangan informasi.
- Tidak ada pola template dari daftar di §4.

---

## 2. Cara evaluasi

### 2.1 Matriks kondisi yang dipotret

Setiap kondisi dipotret di 3 ukuran jendela: **800×600** (minimum), **1120×760** (default), **1920×1080** (maksimal). Tiap ukuran dipotret dua kali: rel kiri penuh dan rel ringkas.

| # | Kondisi | Data contoh |
|---|---|---|
| K1 | Pengguna baru | Belum ada meeting |
| K2 | Normal | 8 meeting selesai, 13 tugas (2 lewat tenggat, 1 hari ini), semua notulen punya intisari |
| K3 | Notulen tanpa intisari | Meeting lama (sebelum langkah 50): baris inti dari ringkasan |
| K4 | Judul panjang | Judul 90 karakter, intisari 300 karakter |
| K5 | Sedang merekam / dijeda | Baris status merah berdenyut |
| K6 | Sedang memproses | Persen bergerak |
| K7 | Ada yang gagal / antrean dijeda | Baris peringatan |
| K8 | Deteksi mati / autostart mati | Baris peringatan di sapaan |
| K9 | Tanya: menunggu, jawaban pendek, jawaban panjang + 4 rujukan, error | — |
| K10 | Tidak ada tugas terbuka | Bagian Tugas hilang |

**Alat:** build frontend + backend tiruan di Edge headless (cara yang sama dengan `feedback2.md`). Hasil disimpan di `docs/eval-beranda/` (tidak di-commit jika besar).

### 2.2 Kriteria penilaian

Skor 0–2 per kriteria:
- 0 = bermasalah
- 1 = cukup
- 2 = baik

| Kelompok | Kriteria |
|---|---|
| **A. Hierarki** | A1 elemen terkuat = Tanya; A2 urutan baca sesuai tugas utama; A3 sapaan tidak mengalahkan isi; A4 jarak antarbagian membedakan kelompok (bukan `gap` seragam) |
| **B. Identitas "Dua suara"** | B1 ada satu hal yang hanya masuk akal di aplikasi ini; B2 warna suara/rekam dipakai sesuai arti (bukan dekorasi); B3 tipografi punya skala jelas, bukan semua 16/18 px |
| **C. Isi & bahasa** | C1 tiap teks punya satu tugas; C2 kata kerja jelas, istilah konsisten dengan glosarium `i18n/id.ts`; C3 keadaan kosong/gagal memberi arah |
| **D. Interaksi** | D1 Tanya: Enter kirim, hasil terbaca, bisa dihapus/diulang; D2 centang tugas terasa (umpan balik), bisa dibatalkan; D3 semua tautan jelas tujuannya |
| **E. Kualitas dasar** | E1 kontras ≥ 4,5:1; E2 fokus keyboard terlihat di semua kontrol (termasuk input Tanya); E3 `prefers-reduced-motion`; E4 tidak ada luapan teks di 800 px; E5 pembaca layar: landmark & label |
| **F. Bebas pola template** | F1–F5 sesuai daftar di §4 |

---

## 3. Temuan awal dari membaca kode (sebelum tangkapan layar)

| # | Temuan | Lokasi | Kriteria |
|---|---|---|---|
| T1 | **Sapaan jadi elemen terbesar** (`h1 text-2xl`), padahal tugas utama adalah bertanya. Kolom Tanya hanya `text-base` dengan bayangan generik. | `+page.svelte:132`, `:147–167` | A1, A3 |
| T2 | **Semua bagian berjarak sama** (`gap-8`). Status, Notulen, dan Tugas terbaca setara; tidak ada kelompok "sekarang" vs "sebelumnya". | `:130` | A4 |
| T3 | **Teks status memakai titik tengah** ("Siap merekam · deteksi meeting aktif"). Ini salah satu pola template. Lebih jelas sebagai satu kalimat: "Meeting Zoom, Teams, dan Meet akan ditawarkan untuk direkam." | `i18n/id.ts` (`beranda.ready`) | C1, F5 |
| T4 | **Kolom Tanya generik** (ikon kaca pembesar + tombol kirim) dan tidak menunjukkan *apa* yang bisa ditanyakan. Tidak ada contoh pertanyaan dari meeting pengguna sendiri. | `:147–167` | B1, C1 |
| T5 | **Jawaban Tanya hilang** saat pindah halaman, dan tidak ada cara menutup/menghapusnya. | `:18–21` | D1 |
| T6 | **Input Tanya `outline-none`**: fokus hanya terlihat lewat warna garis tepi form (`focus-within:border-ink-faint`), kontrasnya lemah. | `:159` | E2 |
| T7 | **Tiga animasi berjalan bersamaan** saat merekam + memproses: denyut titik rekam, putaran ikon, denyut kerangka. Seharusnya hanya satu yang menarik perhatian. | `:190`, `:202`, `:209` | E3, prinsip gerak |
| T8 | **"Notulen terbaru" tidak membawa identitas dua suara.** Kalimat inti datang dari intisari, tetapi tidak jelas apakah itu keputusan, tugas, atau ringkasan. | `:239–252` | B1, B2 |
| T9 | **Centang tugas langsung memuat ulang seluruh Beranda.** Tugas hilang tanpa jeda, tanpa umpan balik, tanpa urungkan. | `toggleTask` | D2 |
| T10 | **Format waktu tidak konsisten:** "Sel 14.00" vs tanggal penuh setelah 6 hari; jam lokal tidak memakai `formatTime` yang sudah ada. | `when()` | C2 |
| T11 | **Batas jam sapaan** pagi < 11, siang < 15, sore < 19 sedikit menyimpang dari kebiasaan (sore 15–18, malam ≥ 18). | `greeting` | C2 |
| T12 | **Baris status seperti "kartu"** (`rounded-xl bg-wash`) dengan bentuk sama untuk merekam, memproses, dan gagal; hanya warnanya yang berbeda. | `:197–226` | A2, F4 |
| T13 | **Di 1920 px, isi `max-w-2xl` di tengah** meninggalkan ±60% layar kosong. Perlu diputuskan: disengaja (fokus) atau kolom kanan untuk tugas. | `:130` | A2 |

---

## 4. Daftar periksa pola template (F)

| # | Pola | Status awal |
|---|---|---|
| F1 | Label huruf kapital berjarak di atas tiap judul | Tidak ada ✅ |
| F2 | Semua isi dipotong jadi kartu identik dengan bayangan abu yang sama | Sebagian: baris status (T12) |
| F3 | Panah "→" di tautan | Tidak ada ✅ ("Semua meeting", "Semua tugas (n)") |
| F4 | Warna aksen dipakai sebagai dekorasi | Perlu dicek di tangkapan layar |
| F5 | Meta dipisah titik tengah | **Ada** (T3) |

---

## 5. Arah perbaikan yang diusulkan (diputuskan setelah evaluasi)

**Satu hal yang berani:** kolom **Tanya** menjadi pusat Beranda. Semua yang lain tenang.

| # | Usulan | Menjawab |
|---|---|---|
| P1 | Sapaan turun jadi baris kecil di atas Tanya (`text-base`, `ink-soft`). Kolom Tanya jadi elemen terbesar (`text-xl`, tinggi ±64 px, tepi tinta tegas saat fokus, tanpa bayangan generik). | T1, T6 |
| P2 | Di bawah kolom: 2–3 **contoh pertanyaan dari meeting pengguna sendiri**, dibangkitkan dari judul/topik terbaru, mis. "Apa keputusan di Sync Mingguan?" atau "Siapa mengerjakan revisi desain?". Klik → langsung bertanya. Pengguna baru: contoh umum. | T4, B1 |
| P3 | Jawaban punya tombol tutup, tetap ada saat kembali ke Beranda (state modul, bukan disimpan permanen), rujukan tampil sebagai chip waktu seperti di detail meeting. | T5, D1 |
| P4 | Dua kelompok jarak: **"Sekarang"** (Tanya + status) rapat, lalu jarak besar, lalu **"Sebelumnya"** (Notulen + Tugas). | T2 |
| P5 | Status sebagai **satu kalimat dengan penanda kecil**, bukan kartu: titik merah untuk merekam (satu-satunya animasi), ikon untuk memproses (diam, persen berubah), garis kiri `bad` untuk gagal. | T7, T12 |
| P6 | Notulen terbaru: kalimat inti diberi awalan jenisnya dalam warna tinta lembut ("Diputuskan: …" / "Ringkasan: …"); hari & jam memakai `formatTime`. | T8, T10 |
| P7 | Centang tugas: coret + pudar 1,5 dtk dengan tombol "Urungkan" sebelum hilang; tanpa memuat ulang seluruh halaman. | T9 |
| P8 | Teks: status siap jadi satu kalimat tanpa titik tengah; batas jam sapaan 11/15/18. | T3, T11 |
| P9 | Layar ≥ 1440 px: tetap satu kolom (fokus), tetapi Tugas mendesak boleh pindah ke kolom kanan sempit agar tidak di bawah lipatan. Diputuskan dari tangkapan layar K2 di 1920 px. | T13 |

---

## 6. Langkah

| Langkah | Isi | Hasil |
|---|---|---|
| **E1 — Siapkan data contoh & potret** | Backend tiruan dengan K1–K10; potret 3 ukuran × 2 kondisi rel × 10 kondisi | Folder tangkapan layar + catatan per kondisi |
| **E2 — Nilai** | Skor kriteria §2.2 per kondisi; konfirmasi/koreksi T1–T13; tambah temuan baru | Tabel skor di file ini (bagian §7) |
| **E3 — Putuskan** | Pemilik memilih usulan P1–P9 yang dipakai (terutama P2 dan P9) | Daftar usulan disetujui |
| **E4 — Perbaiki** | Kerjakan usulan yang disetujui, satu commit per kelompok (hierarki & Tanya; status & jarak; notulen & tugas; teks) | Commit + catatan `CLAUDE.md` |
| **E5 — Potret ulang** | Ulangi E1–E2 untuk kondisi yang terdampak; bandingkan skor sebelum/sesudah | Skor naik di A, B, D, F tanpa turun di E |

**Uji pemilik setelah E4:**
- Buka aplikasi: mata langsung ke kolom Tanya.
- Klik contoh pertanyaan: jawaban + rujukan muncul.
- Centang tugas bisa diurungkan.
- Saat merekam hanya ada satu animasi.
- Di 800×600 tidak ada teks terpotong.

---

## 7. Hasil

Atas permintaan pemilik (8 Okt 2026), langkah E1–E3 (memotret & menilai) dilewati. Usulan **P1–P8 langsung dikerjakan**. **P9 tidak dipakai:** Beranda tetap satu kolom di semua lebar layar.

Tangkapan layar & skor (E5) belum dibuat; uji manual pemilik memakai daftar di §6.
