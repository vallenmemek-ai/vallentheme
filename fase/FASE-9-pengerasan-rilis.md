# FASE 9 — Pengerasan & Rilis

**Baca dulu:** semua `panduan/`, `docs/PROGRESS.md`, `docs/ARCH.md`.

## Tujuan
Siap dipublikasikan: stabil 24 jam, kebal terhadap kondisi ekstrem, aman, terdokumentasi, dan terpaket (.deb lalu Flatpak). Rilis **v0.1.0**.

## Langkah rinci

### 9.1 Soak test 24 jam
Gunakan `scripts/soak.sh`. Skenario selama 24 jam: wallpaper video 1080p (dan satu periode 4K bila perangkat mampu), ganti wallpaper tiap 10 menit, buka-tutup GUI berkala, jeda/lanjut oleh window fullscreen, suspend/resume sekali, kunci/buka layar beberapa kali.
Catat: RSS daemon/mpv/GUI per menit, jumlah file descriptor (`ls /proc/<pid>/fd | wc -l`), jumlah thread, suhu bila ada. **Kriteria lulus:** tidak ada crash, RSS tidak naik terus (tren datar ± 10%), FD/thread stabil, tidak ada proses yatim.
Bila ditemukan kebocoran: perbaiki dan ulangi sebagian soak (minimal 4 jam) sampai stabil.

### 9.2 Uji kekacauan (chaos)
Setiap skenario harus berakhir dengan **pesan jelas dan pemulihan**, tanpa panic:
- Disk penuh (tulis DB/cache/state), `/tmp` penuh.
- DB terkunci, korup, versi lebih baru; direktori data tidak bisa ditulis.
- File wallpaper dihapus/dipindah saat diputar; drive eksternal dicabut.
- `mpv`/`ffmpeg`/`ffprobe`/`yt-dlp` tidak ada; versi mpv lama.
- Tanpa X11 (Wayland, tanpa DISPLAY); koneksi X putus mendadak.
- D-Bus: daemon dibunuh, bus restart, dua GUI sekaligus.
- Monitor dicabut/dipasang/diubah resolusi berulang cepat (50×).
- Memori rendah (batasi dengan `ulimit -v`/cgroup) dan CPU sibuk penuh.
- Jam sistem melompat; locale berbeda; `HOME` dengan spasi/unicode.
- Impor 10.000 file; nama file aneh; symlink loop.
- Sinyal `SIGTERM`/`SIGINT`/`SIGHUP` pada GUI dan daemon.

### 9.3 Katalog error final
Tabel di `docs/ERRORS.md`: kode, kapan terjadi, pesan Indonesia untuk pengguna (apa + solusi), pesan teknis untuk log. Tes yang memastikan semua varian `Error` punya entri dan pesan tidak kosong.

### 9.4 Keamanan & dependensi
- `cargo audit` dan `cargo deny check` (lisensi, advisories, duplikat). Perbaiki atau catat pengecualian beralasan.
- Tinjau semua titik eksekusi proses (tanpa shell, argumen array, `--`), validasi path/URL, izin berkas runtime (`0700`/`0600`), tidak ada data sensitif di log.
- Pastikan tidak ada `unwrap/expect/panic` tersisa di kode non-test (clippy sudah memblokir; tinjau `unsafe` yang tersisa dan dokumentasikan).
- Audit cakupan test untuk modul kritis (supervisor, kebijakan, scanner, config).

### 9.5 Lisensi (butuh keputusan pemilik proyek)
**Berhenti dan kirim BLOKIR** meminta keputusan lisensi sebelum rilis. Pertimbangan yang perlu dijelaskan kepada pemilik:
- mpv: lisensi GPLv2+ atau LGPLv2.1+ tergantung build. Aplikasi memanggil mpv sebagai **proses terpisah** (bukan menautkan libmpv), sehingga keterkaitan lisensinya lebih longgar, tetapi tetap perlu dicatat.
- GTK4/libadwaita: LGPL-2.1+. Ikon Adwaita: LGPL-3.0/CC-BY-SA-3.0.
- Crate Rust: periksa lisensi lewat `cargo deny`/`cargo license`.
Tulis `LICENSE`, `THIRD_PARTY_LICENSES.md` (dihasilkan dari `cargo about`/`cargo license` + ikon + font bila ada).

### 9.6 Packaging
**.deb (prioritas):**
- Gunakan `cargo-deb` atau skrip `dpkg-deb`. Paket `vallentheme` memasang: `vallentheme-gui`, `vallentheme-daemon`, `.desktop` aplikasi, berkas service D-Bus, ikon (hicolor), metainfo, terjemahan.
- `Depends`: `libgtk-4-1`, `libadwaita-1-0`, `mpv`, `ffmpeg`, `libsqlite3-0`. `Recommends`: `yt-dlp`. `Suggests`: `i965-va-driver`/`intel-media-va-driver`/`mesa-va-drivers` (decode hardware).
- Skrip `postinst`/`prerm`: tidak mengubah setting pengguna; `prerm` menghentikan daemon yang berjalan. Penghapusan **tidak** menghapus data pengguna (library/config) kecuali `purge` dengan konfirmasi dokumentasi.
- Uji pasang/hapus bersih di Debian 13 segar (VM/container dengan X11) dan di Ubuntu LTS terbaru bila ada.

**Flatpak (lanjutan, boleh setelah .deb stabil):**
- Runtime `org.gnome.Platform` (versi yang punya libadwaita ≥ 1.5). Izin: `--socket=x11`, `--device=dri`, akses folder media (`--filesystem=xdg-videos:ro` dan `xdg-pictures:ro` atau portal file), `--talk-name` bila perlu D-Bus layanan desktop, jaringan hanya bila fitur tautan diaktifkan.
- Tantangan: daemon latar di sandbox, autostart (portal Background), memanggil `gsettings` desktop (butuh `--talk-name=org.freedesktop.portal.*` atau `--filesystem=xdg-run/dconf` — kaji dan **tanyakan** bila menyulitkan), membundel mpv/ffmpeg/yt-dlp sebagai modul.
- Bila Flatpak terlalu banyak kompromi, tetapkan sebagai target v0.2 (catat di `IDEAS.md`).

### 9.7 Dokumentasi
- `README.md`: apa itu, tangkapan layar, fitur, instalasi (.deb), persyaratan (X11, mpv), penggunaan singkat, **pemecahan masalah** (mpv tidak ada, Wayland, ikon desktop tertutup, performa), batasan yang diketahui.
- `docs/USER_GUIDE.md` (Indonesia) dan `docs/CONTRIBUTING.md`, `docs/ERRORS.md`, `CHANGELOG.md` (format Keep a Changelog), kebijakan privasi singkat ("tidak ada telemetri").
- `docs/ARCH.md` dirapikan sebagai dokumen arsitektur final.

### 9.8 Kualitas akhir UI
- Telusuri semua layar di 420/640/960/1440 px, skala 1x/2x, font besar.
- Periksa kontras, fokus keyboard, tooltip, terjemahan (tidak ada string Inggris tersisa).
- Periksa perilaku saat jaringan mati, saat tanpa library, saat library besar.
- Ikon aplikasi (SVG) dan ikon simbolik lengkap; metainfo AppStream valid (`appstreamcli validate`).

### 9.9 Rilis
- Versi `0.1.0` di semua crate, `CHANGELOG.md`, tag git `v0.1.0`.
- Bangun rilis (`cargo build --release`), uji biner rilis (bukan hanya debug).
- Hasilkan `.deb`, hitung checksum (`sha256sum`), tulis catatan rilis.

## Gate
- [ ] Soak 24 jam lulus (RSS datar, FD/thread stabil, tanpa crash/yatim); data tercatat di `docs/PROGRESS.md`
- [ ] Semua skenario chaos berakhir dengan pesan jelas tanpa panic
- [ ] `docs/ERRORS.md` lengkap dan teruji
- [ ] `cargo audit` dan `cargo deny` bersih (pengecualian beralasan tercatat)
- [ ] Keputusan lisensi diterima dari pemilik proyek; `LICENSE` dan `THIRD_PARTY_LICENSES.md` ada
- [ ] `.deb` terpasang bersih di Debian 13 segar dan terhapus bersih (data pengguna tetap, kecuali purge)
- [ ] README, USER_GUIDE, CHANGELOG ada dan akurat
- [ ] Biner rilis diuji manual penuh (semua layar, semua fitur)
- [ ] `scripts/check.sh` hijau; tag `v0.1.0` dibuat

## Artefak
`.deb`, (opsional) manifest Flatpak, dokumentasi, catatan rilis, laporan soak.

## Jangan lakukan
Merilis tanpa soak 24 jam lulus. Memilih lisensi sendiri. Menyembunyikan batasan yang diketahui dari README.
