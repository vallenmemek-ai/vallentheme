# FASE 8 — Fitur Tambahan

**Baca dulu:** `panduan/05` (bagian 1–3), `panduan/06` (bagian 6, 9, 10), `panduan/03` (bagian 7, 9).

## Tujuan
Melengkapi fitur sesuai referensi Fresco: **Dari tautan**, folder terpantau, halaman Pengaturan lengkap, menu `⋯` lengkap, dan notifikasi.

## Langkah rinci

### 8.1 Dari tautan (yt-dlp)
- Dialog `adw::Dialog`: kolom URL (validasi langsung), **kualitas maksimum** (ComboRow: 720p/1080p/1440p/2160p/Terbaik), **folder tujuan** (bawaan `storage.download_dir`), tombol Batal/Unduh.
- Validasi URL (`panduan/00` bagian 3): `http/https`, ≤ 2048 karakter, tidak diawali `-`. Tampilkan pesan bila tidak valid.
- Jalankan `yt-dlp` sesuai `panduan/06` bagian 6 di thread kerja: baca progres per baris, tampilkan progres bar + kecepatan bila tersedia, tombol **Batal** (`SIGTERM`, tunggu 2 detik, `SIGKILL`, hapus `.part`).
- Selesai: impor file hasil lewat `core::scan` (otomatis ke library), toast "Ditambahkan: <judul>".
- Batas keselamatan: `--max-filesize` dari pengaturan (bawaan 2 GB), satu unduhan aktif sekaligus (antre berikutnya).
- Bila `yt-dlp` tidak terpasang: dialog "yt-dlp belum terpasang" + perintah pasang; tombol "Dari tautan" tetap tampil tetapi menjelaskan.
- Playlist: `--no-playlist` (hanya satu video).
- Error umum → pesan ramah: jaringan putus, video privat/dibatasi wilayah, format tidak tersedia, disk penuh, URL tidak didukung.
- Hak cipta: tambahkan catatan kecil di dialog "Pastikan kamu berhak memakai konten ini." (tanpa ceramah).

### 8.2 Folder terpantau (opsional per folder)
- Saat menambah folder, opsi "Pantau perubahan folder".
- `notify` (inotify) di GUI atau layanan ringan: file baru → impor otomatis (batch, debounce 2 detik); file hilang → tandai `missing`.
- Batas jumlah folder terpantau (bawaan 10) dan penggunaan watch; bila inotify habis, turun ke pemindaian berkala (tiap 5 menit) dengan peringatan log.

### 8.3 Pengaturan lengkap
Selesaikan semua halaman di `panduan/05` bagian 1 (Umum, Penyimpanan, Tentang; halaman Kinerja dari Fase 7):
- **Jalankan saat login** → menulis/menghapus berkas autostart (Fase 5).
- **Bahasa** → simpan, beri toast "Mulai ulang aplikasi untuk menerapkan".
- **Penyimpanan** → ukuran cache dihitung di latar, **Bersihkan** (hapus hanya isi cache; **tidak** menyentuh library atau file pengguna), batas cache, lokasi unduhan.
- **Setel ulang pengaturan** → hanya config; library tetap.
- Perubahan tersimpan segera (atomik).

### 8.4 Menu `⋯` lengkap
Pengaturan, Tentang, Hentikan wallpaper, Keluar, Keluar dan hentikan daemon. Pastikan "Keluar" tidak mematikan wallpaper.

### 8.5 Notifikasi
Notifikasi sistem ringan (`gio::Notification`) hanya untuk kegagalan penting yang terjadi saat GUI tertutup (mis. daemon masuk `Failed`). Satu notifikasi per kejadian, tidak berulang. Dapat dimatikan di Pengaturan.

### 8.6 Poles kecil
- Tooltip, label aksesibel, urutan Tab diperiksa ulang di semua layar.
- Pesan kosong/error konsisten dengan `panduan/05`.
- Semua string baru lewat `tr!` dan terjemahan Indonesia.

## Kasus tepi
- URL valid tetapi bukan video (halaman biasa) → pesan jelas.
- Unduhan lama lalu GUI ditutup: batalkan dengan bersih (tidak ada proses yatim) atau lanjutkan hingga selesai — **pilih batalkan dan bersihkan**, beri konfirmasi bila ada unduhan berjalan.
- Dua unduhan URL yang sama (deteksi dan tolak).
- Nama file hasil unduhan bentrok (yt-dlp menambah id; jangan menimpa).
- Folder tujuan tidak bisa ditulis → pesan sebelum mulai.
- Folder terpantau dilepas (unmount) → tandai `missing`, tanpa crash.
- Cache sangat besar (puluhan GB): pembersihan di latar dengan progres.
- Karakter berbahaya pada URL (`;`, `$()`, spasi, awalan `-`) → ditolak/aman karena tanpa shell dan tanda `--`.

## Tes
- Unit: validasi URL (kumpulan kasus berbahaya), parser progres yt-dlp, perhitungan ukuran cache.
- Integrasi: `yt-dlp` palsu (skrip yang mencetak progres dan bisa gagal) untuk menguji alur, pembatalan, pembersihan `.part`.
- Manual: unduh video nyata kecil, impor, jadikan wallpaper.

## Gate
- [ ] Unduhan gagal/dibatalkan tidak meninggalkan file setengah jadi
- [ ] URL berbahaya ditolak atau tidak dieksekusi sebagai perintah (diuji)
- [ ] Pembersihan cache tidak menyentuh file asli/library
- [ ] Folder terpantau menambah/menandai hilang dengan benar
- [ ] Semua opsi Pengaturan berfungsi dan bertahan setelah restart
- [ ] Menutup GUI saat mengunduh tidak meninggalkan proses yatim
- [ ] `scripts/check.sh` hijau

## Artefak
Dialog Dari tautan, pemantau folder, Pengaturan lengkap, notifikasi.

## Jangan lakukan
Menjalankan `yt-dlp` lewat shell. Menyimpan unduhan di luar folder yang dipilih. Menambah fitur unduhan lanjutan (playlist, antrean banyak) — masuk `IDEAS.md`.
