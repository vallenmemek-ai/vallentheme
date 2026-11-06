# FASE 7 — Sistem Kinerja (Otomatis + Manual)

**Baca dulu:** `panduan/05` (bagian 1, halaman Kinerja), `panduan/06` (bagian 1, 2, 5, 8), hasil ukur Fase 1 di `docs/ARCH.md`.

## Tujuan
Aplikasi tetap stabil di **perangkat apa pun** karena mengukur dan beradaptasi, bukan menebak, sementara pengguna tetap bisa mengatur sendiri. Karena app akan dipublikasikan, jangan menanam asumsi spek mesin pengembang. (Catatan: Fresco memutar 4K lancar di mesin pengembang, jadi **jangan batasi kualitas secara default**.)

## Prinsip
1. Default **Otomatis**: mulai dari kualitas asli, turun hanya bila terbukti perlu.
2. Semua keputusan **terlihat** (log keputusan) dan **dapat dikalahkan** pengguna (mode lain/kustom).
3. Histeresis: hindari naik-turun bolak-balik.
4. Pemantau sendiri harus ringan (< 1% CPU).
5. Logika keputusan = **fungsi murni** di `core` agar mudah di-test.

## Mode
| Mode | Perilaku |
|---|---|
| Otomatis (bawaan) | Adaptif lewat level L0–L4 (di bawah) |
| Kualitas asli | Tidak ada batas apa pun; aturan jeda otomatis tetap sesuai pengaturan pengguna |
| Seimbang | Tetap di L1 (≤ 30 fps), hwdec otomatis |
| Hemat | Tetap di L2/L3 (24 fps, render ≤ 720p), jeda agresif |
| Kustom | Nilai manual dari halaman Kinerja |

### Level adaptasi (mode Otomatis)
| Level | Penjelasan | Tindakan |
|---|---|---|
| L0 | Asli | Tanpa batas |
| L1 | Batas fps | `--vf=fps=30` |
| L2 | Hwdec aman + 24 fps | Paksa mode hwdec yang terbukti (atau software) + `fps=24` |
| L3 | Turunkan resolusi render | `--vf=...,scale=-2:<H>` (H = 1080/720). Catatan: dengan hwdec mungkin butuh `auto-copy` (lihat Fase 1) |
| L4 | Jeda | `pause=yes` (bingkai terakhir tampil) sampai kondisi membaik atau pengguna melanjutkan |

## Langkah rinci

### 7.1 Deteksi perangkat (`core::perf::profile`)
Dijalankan sekali saat daemon pertama start (hasil di-cache, bisa diulang manual):
- Jumlah core, RAM total, model CPU/GPU (informasi), ada baterai atau tidak, sesi (X11/Wayland).
- **Uji hwdec nyata**: buat klip H.264 kecil (ffmpeg, 2 detik, 1280×720) di cache, putar sekilas dengan `mpv --vo=null --frames=24 --hwdec=<kandidat> --msg-level=all=no` dan baca `hwdec-current` lewat IPC. Kandidat: `vaapi`, `nvdec`, `auto-safe`. Simpan mana yang terbukti bekerja.
- Hasil → `DeviceProfile` (JSON) untuk `GetDeviceProfile` dan dialog di Pengaturan.

### 7.2 Pemantau runtime (`daemon`)
Interval 2 detik (naik ke 5 detik saat stabil lama):
- Dari mpv (IPC): `vo-drop-frame-count`, `decoder-frame-drop-count`, `estimated-vf-fps`, `container-fps`, `hwdec-current`.
- Dari `/proc`: CPU proses (daemon+mpv), CPU total, `MemAvailable`.
- Hitung **laju drop** = delta drop / delta frame yang seharusnya tampil (`fps_target × dt`).
Hasil → struct `Metrics` dikirim ke kebijakan.

### 7.3 Kebijakan (`core::perf::policy`)
Fungsi murni: `decide(state, metrics, config, now) -> Decision`.
- **Turun** satu level bila: laju drop > 5% **selama ≥ 10 detik berturut-turut** atau CPU total > 90% selama 15 detik atau `MemAvailable` < 150 MB.
- **Naik** satu level bila stabil (drop < 1%, CPU < 70%) selama **≥ 60 detik**.
- Dwell time: minimal 30 detik antar perubahan; maksimal 1 perubahan per 15 detik.
- Setelah tiga naik-turun berpasangan dalam 10 menit, tahan level lebih rendah selama 30 menit (antiosilasi).
- Level maksimum yang diizinkan dibatasi mode/pengaturan (mis. Kualitas asli = selalu L0).
- Bila mode Kustom: terapkan nilai pengguna, tanpa adaptasi.
- Keluaran `Decision { new_level, reason_code, message_id, params }`.
Test unit dengan deret `Metrics` sintetis (naik, turun, osilasi, lonjakan sesaat, jeda panjang).

### 7.4 Penerapan keputusan
Daemon menerjemahkan `Decision` ke perubahan mpv (properti hidup bila bisa; restart terkendali bila tidak). State daemon menjadi `Degraded(level)` bila level > 0. `StateChanged` dikirim. Entri ditulis ke **log keputusan** (ring buffer 200 entri; ditulis juga ke log berkas).

### 7.5 Aturan jeda otomatis
Sumber peristiwa (Fase 1 + `panduan/06` bagian 3, 8):
- **Fullscreen** pada monitor yang sama (atau semua monitor bila tidak bisa dibedakan).
- **Maximize** (opsional, bawaan nyala).
- **Baterai** < ambang dan tidak mengisi daya.
- **Layar terkunci/mati** (sinyal ScreenSaver, DPMS).
- **Mode hemat daya sistem** (bila tersedia).
Setiap aturan dapat dimatikan di Pengaturan. Jeda = `pause=yes` + state `Paused(reason)`; lanjut otomatis saat kondisi hilang (debounce 500 ms agar tidak berkedip). Pengaturan per wallpaper (`power`) menimpa bawaan global (`panduan/04` bagian 3).
Pastikan CPU saat jeda mendekati 0% (mpv `pause` + tanpa pemantau berat).

### 7.6 Prioritas proses
Terapkan `nice` untuk mpv sesuai pengaturan (`setpriority` via `libc`, dibungkus fungsi aman). Default Normal.

### 7.7 UI halaman Kinerja
Implementasi persis `panduan/05` bagian 1 (halaman Kinerja): mode, hwdec (opsi yang tidak tersedia di `DeviceProfile` dinonaktifkan dengan keterangan), batas fps/resolusi, aturan jeda, nice, optimasi saat impor, dan dua dialog diagnostik (log keputusan, profil perangkat). `SetPerformance` mengirim konfigurasi ke daemon (simpan juga di `config.toml`).
Subteks tiap mode menjelaskan efeknya dengan bahasa awam.

### 7.8 Optimalkan saat impor (opsional, **mati** secara default)
- Bila nyala: setelah impor video dengan resolusi/fps di atas target, buat salinan optimal di `~/.cache/vallentheme/optimized/` dengan perintah di `panduan/06` bagian 5 (progres dari `-progress pipe:1`, bisa dibatalkan, satu pekerjaan sekaligus, prioritas `nice` rendah).
- Entri wallpaper menyimpan `optimized_path`; pemutaran memakai salinan itu **bila ada dan pengguna mengaktifkan**; file asli tidak disentuh. Tombol "Gunakan file asli" per wallpaper di layar Edit (baris tambahan hanya muncul bila ada salinan).
- Batas ruang disk (`cache_limit_mb`) dipatuhi.

### 7.9 Log keputusan (UI)
Dialog daftar: waktu, level lama → baru, alasan manusiawi ("Menurunkan ke 30 fps karena 12% frame terlewat selama 10 detik"). Tombol "Salin" dan "Kosongkan".

## Kasus tepi
- mpv belum siap/properti belum ada saat awal: abaikan sampel pertama (3 detik pemanasan).
- Laju drop sesaat saat ganti wallpaper (abaikan 5 detik pertama).
- Perubahan mode saat sedang di level tinggi (reset ke L0 atau ke batas mode baru).
- Laptop dicabut/dipasang charger berulang (debounce).
- Monitor berbeda kinerja (keputusan per monitor, bukan global) — `Decision` per monitor.
- Sistem sedang kehabisan memori berat (jeda cepat, jangan memicu OOM).
- Video dengan fps variabel atau fps asli sangat rendah (target fps = min(fps video, batas)).
- Sinyal ScreenSaver tidak tersedia di desktop pengguna (fitur tersembunyi, tidak error).

## Tes
- Unit kebijakan (tabel skenario) dan pemetaan level → opsi mpv.
- Integrasi dengan mpv palsu yang melaporkan laju drop yang bisa diatur.
- Manual: jalankan beban CPU (`stress`/`yes > /dev/null` beberapa proses) → app turun level, lalu kembali setelah beban selesai.

## Gate
- [ ] Unit test kebijakan hijau (turun, naik, histeresis, antiosilasi)
- [ ] Simulasi beban tinggi → app turun tanpa crash, lalu kembali ke L0 setelah stabil ≥ 60 detik
- [ ] Mode **Kualitas asli** benar-benar tidak membatasi apa pun
- [ ] CPU saat jeda (fullscreen) mendekati 0%
- [ ] Pemantau memakai CPU < 1% (diukur)
- [ ] Log keputusan terbaca dan akurat; profil perangkat tampil
- [ ] Optimasi saat impor mati default; saat nyala tidak menyentuh file asli dan bisa dibatalkan
- [ ] Semua opsi manual punya efek nyata
- [ ] `scripts/check.sh` hijau

## Artefak
`core::perf`, pemantau di daemon, halaman Kinerja, dialog diagnostik, hasil ukur di `docs/PROGRESS.md`.

## Jangan lakukan
Membatasi kualitas secara diam-diam saat mode Kualitas asli. Memakai ambang yang ditanam untuk mesin tertentu tanpa dapat dikonfigurasi.
