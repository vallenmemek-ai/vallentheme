# FASE 6 — Halaman Edit Wallpaper

**Baca dulu:** `panduan/04-gui-layar-edit.md` (seluruhnya), `panduan/06-referensi-teknis.md` (bagian 2, 4), `panduan/02` (6.4, 6.6).

## Tujuan
Semua pengaturan per wallpaper bekerja nyata: pengisian, transisi, bisu/volume, hemat daya, putar, pangkas, pilih monitor, dan tombol "Jadikan wallpaper".

## Ruang lingkup
**Termasuk:** layar Edit lengkap, penyimpanan otomatis `settings_json`, penerapan langsung via daemon (`Update`), pratinjau yang mensimulasikan pengaturan.
**Tidak termasuk:** sistem kinerja otomatis (Fase 7) — baris "Hemat daya" hanya **menyimpan** pilihan; efek jedanya diimplementasi di Fase 7 (catat di `docs/PROGRESS.md`).

## Langkah rinci

### 6.1 Struktur layar
Bangun sesuai pohon widget `panduan/04` bagian 2. Halaman dibuat saat item dibuka dan dibuang saat kembali (hindari menyimpan state besar).

### 6.2 Pengikatan data
- `EditViewModel` (GObject) memegang `WallpaperSettings` + sinyal perubahan. Widget terhubung dua arah.
- Setiap perubahan: validasi (klem) → update pratinjau → jadwalkan simpan (debounce 400 ms) → bila wallpaper aktif, kirim `Update`.
- Opsi yang tidak berlaku disembunyikan/dinonaktifkan (`panduan/04` bagian 3): foto vs video, tanpa audio, Ubin hanya foto.

### 6.3 Pratinjau
- Frame resolusi tinggi 960 px dibuat di latar (core `preview_frame`), thumbnail ditampilkan lebih dulu.
- Rasio frame mengikuti rasio monitor target.
- Simulasi fill/rotasi/zoom/pan sesuai `panduan/04` bagian 4–5. Gunakan gambar pra-putar (pixbuf) untuk rotasi.
- Interaksi geser/zoom: `gtk::GestureDrag` + `gtk::EventControllerScroll` + `GestureZoom`; klem pan agar tepi tidak kosong; aktif hanya pada mode Penuhi.
- Throttle pembaruan (≤ 60 fps), hindari render ulang berat.

### 6.4 Terjemahan pengaturan → mpv/setter
- Video: pemetaan `panduan/06` bagian 2 (fill, rotasi, zoom, pan, mute, volume).
- Foto: `picture-options` (bagian 4) dan, bila ada rotasi/pangkas, buat file hasil di cache (`~/.cache/vallentheme/rendered/<hash>.jpg`, via ffmpeg `transpose`/`crop`/`scale`) lalu setel file itu. File asli tidak diubah.
- Fungsi murni `settings_to_mpv(&WallpaperSettings, &MonitorInfo) -> MpvOptions` dengan **test unit** untuk semua kombinasi fill/rotasi/zoom.

### 6.5 Perintah `Update` di daemon
Implementasi di daemon: `Update(patch_json)` mengubah properti mpv lewat `set_property` (volume, mute, panscan, video-zoom, video-pan-*, video-rotate, keepaspect, video-unscaled) tanpa restart. Properti yang tidak bisa diubah hidup → restart mpv terkendali (tanpa kedip berlebihan; bila ada transisi, gunakan).

### 6.6 Transisi
- **Tidak ada**: ganti langsung.
- **Pudar**: coba lewat window baru di bawah window lama + opasitas (`_NET_WM_WINDOW_OPACITY`) hanya bila compositor aktif (deteksi `_NET_WM_CM_S0` selection). Batas waktu implementasi 1 hari kerja; bila tidak stabil → tetapkan "Pudar" sebagai **eksperimental/dinonaktifkan** dan sembunyikan opsinya (catat di BLOKIR/PROGRESS). Jangan mengorbankan stabilitas demi efek.
- Durasi pudar 400 ms, tidak lebih.

### 6.7 Tombol "Jadikan wallpaper"
Perilaku `panduan/04` bagian 6: spinner, toast sukses (dengan **Urungkan**: kembali ke wallpaper sebelumnya bila ada di state daemon), dialog/pesan error berkode, "Terapkan ulang" bila sudah aktif.
- Multi-monitor: `ComboRow` Monitor muncul bila > 1; ingat pilihan terakhir per wallpaper (`settings_json.monitor`).
- Perbarui `last_used_at` dan penanda "Dipakai" di Library.

### 6.8 Menu & titik masuk
- Kartu Library → klik membuka Edit. Menu kartu "Jadikan wallpaper" kini memakai pengaturan tersimpan.
- Tombol kembali (`Esc`) menyimpan segera (tanpa menunggu debounce).

### 6.9 Pengaturan bawaan
Item baru memakai bawaan: fill=Penuhi, transisi=Tidak ada, mute=**true** (hindari suara mengejutkan), volume=50, power=Default, rotasi/zoom/pan=0.

## Kasus tepi
- Wallpaper dihapus/hilang saat layar Edit terbuka.
- Video tanpa audio; foto animasi; foto sangat besar (> 8000 px); rasio ekstrem (panorama 32:9, potret 9:16).
- Rotasi 90° pada rasio berbeda dari monitor (cek hasil mode Penuhi/Pas).
- Mengubah pengaturan sangat cepat (slider volume digeser terus): hanya nilai terakhir yang disimpan/dikirim (throttle).
- Daemon mati di tengah pengeditan: perubahan tetap tersimpan, toast jelas, aksi terapkan bisa diulang.
- Monitor dicabut saat memilih monitor.
- Ubah mode ke Pas setelah zoom/pan: nilai disimpan, interaksi nonaktif.

## Tes
- Unit: `settings_to_mpv`, klem pan/zoom, pemetaan `picture-options`, serialisasi settings (toleran field hilang).
- Integrasi: `Update` ke daemon dengan mpv palsu memverifikasi urutan `set_property`.
- Manual: semua baris punya efek nyata dan bertahan setelah restart aplikasi.

## Gate
- [ ] Setiap opsi punya efek nyata (video & foto) dan tersimpan setelah GUI/daemon dimulai ulang
- [ ] Nilai tidak valid dikoreksi (tidak ada crash/nilai liar)
- [ ] Mengganti wallpaper 100× berulang: tidak ada kebocoran proses/memori (RSS stabil, `pgrep mpv` = jumlah monitor)
- [ ] Pratinjau cukup mulus (geser/zoom) di renderer cairo
- [ ] **Urungkan** mengembalikan wallpaper sebelumnya
- [ ] Opsi Pudar stabil, atau disembunyikan dengan catatan jelas
- [ ] Layar benar pada lebar 420/640/960 px
- [ ] `scripts/check.sh` hijau

## Artefak
Layar Edit lengkap, `settings_to_mpv`, perintah `Update` di daemon, catatan di `docs/PROGRESS.md`.

## Jangan lakukan
Menimpa file asli saat rotasi/pangkas. Menambah efek di luar daftar.
