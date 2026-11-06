# FASE 1 — Spike Teknis: Render Video ke Desktop (X11)

**Baca dulu:** `panduan/01-arsitektur.md` (bagian 3–5), `panduan/06-referensi-teknis.md` (bagian 2, 3, 8).

## Tujuan
Membuktikan bagian paling berisiko **sebelum** membangun UI: video bisa tampil sebagai wallpaper di **MATE + X11**, stabil, hemat, dan bersih saat dimatikan. Kode spike boleh dibuang atau dirapikan; **pengetahuannya wajib dicatat**.

## Aturan fase ini
- Boleh pakai kode eksperimen (`examples/` di crate `render`), tetapi tetap lolos clippy.
- Fokus pada **bukti dan angka**, bukan keindahan kode.
- Jika jalur tidak layak → **berhenti dan tulis BLOKIR**. Jangan lanjut ke Fase 2.

## Langkah rinci

### 1.1 Window desktop minimal
Di `crates/vallentheme-render/examples/spike_window.rs` (x11rb):
1. Hubungkan ke X, ambil layar/root, geometri monitor lewat RandR.
2. Buat window sebesar monitor pertama, tanpa dekorasi.
3. Set atom sesuai `panduan/06-referensi-teknis.md` bagian 3 (tipe DESKTOP, state BELOW/STICKY/SKIP_*, `_NET_WM_DESKTOP=0xFFFFFFFF`).
4. Map, `lower_window`, isi warna solid (cek bahwa window di belakang semua aplikasi dan tidak muncul di panel/taskbar MATE).

### 1.2 Menempelkan mpv
`spike_mpv.rs`: jalankan `mpv` sesuai argumen dasar (`panduan/06-referensi-teknis.md` bagian 2) dengan `--wid=<XID window 1.1>`.
- Uji `--vo=gpu` dan `--vo=x11`. Catat mana yang bekerja di GM45 (OpenGL 2.1). Jika `--vo=gpu` gagal/lambat, catat `--gpu-context`/`--vo` yang berhasil.
- Uji `--hwdec=no`, `--hwdec=vaapi`, `--hwdec=auto-safe`, dan `auto-copy` (penting untuk filter `scale`/`fps` di Fase 7). Catat mana yang aktif lewat properti `hwdec-current`.

### 1.3 Kontrol lewat IPC
Klien IPC sederhana: kirim `get_property pause`, `set_property pause true/false`, `set_property volume`, `get_property estimated-vf-fps`, `get_property vo-drop-frame-count`. Verifikasi pencocokan `request_id` dan perilaku saat mpv mati (soket putus).

### 1.4 Interaksi dengan Caja (MATE)
Periksa **dengan ikon desktop tampil**:
- Apakah window kita menutupi ikon Caja? Apakah ikon masih bisa diklik dan di-drag?
- Apakah klik kanan di desktop memunculkan menu Caja?
Coba solusi berurutan (daftar di `06-referensi-teknis.md` bagian 3: a → d). Catat solusi yang berhasil, efek sampingnya, dan cara memulihkan setting yang diubah. Uji juga saat Caja dimulai ulang (`caja -q && caja`) atau tidak berjalan.

### 1.5 Multi-monitor dan perubahan layar
Bila hanya ada satu monitor: simulasikan dengan `xrandr --fb`/`--output ... --mode ...` (ganti resolusi) dan, bila memungkinkan, `xrandr --setmonitor` untuk monitor virtual. Pastikan window mengikuti perubahan (resize/relokasi) tanpa crash.

### 1.6 Deteksi window fullscreen/maximize
Pantau `PropertyNotify` pada root untuk `_NET_ACTIVE_WINDOW`/`_NET_CLIENT_LIST_STACKING`, baca `_NET_WM_STATE` window aktif. Cetak log saat ada window fullscreen atau maximize muncul/hilang. Ukur latensi deteksi.

### 1.7 Pembersihan dan ketahanan
- Daemon mati normal → mpv mati, window tertutup.
- `kill -9` pada program spike → mpv **ikut mati** (`PR_SET_PDEATHSIG`) dan window hilang (X menutup window milik klien yang mati).
- `kill -9` pada mpv → program spike mendeteksi (soket putus/`wait`) dalam < 1 detik dan memulai ulang.
- Tidak boleh ada proses yatim atau window sisa. Cek `pgrep mpv`, `xwininfo -root -tree`.

### 1.8 Ukur kinerja
Tabel pengukuran (isi di `docs/ARCH.md`), tiap sel: CPU% rata-rata 60 detik dan RSS (MB) daemon+mpv.

| Video | hwdec | vo | CPU% | RSS | frame drop/60 dtk |
|---|---|---|---|---|---|
| 1080p H.264 24fps | no | gpu | | | |
| 1080p H.264 24fps | vaapi | gpu | | | |
| 4K H.264 24fps | auto-safe | gpu | | | |
| 4K H.264 24fps | no | x11 | | | |
| 1080p VP9 | auto-safe | gpu | | | |
| Gambar diam (foto) via mpv | — | gpu | | | |
Kondisi: ikon desktop tampil, tidak ada aplikasi berat lain. Catat juga suhu bila mudah (opsional).

Catatan konteks: Fresco di mesin pengembang memutar 4K dengan lancar, jadi **jangan berasumsi 4K mustahil**; ukur dan catat.

### 1.9 Putusan teknis
Tulis di `docs/ARCH.md` bagian "Keputusan render X11":
- Metode terbaik yang dipakai (window DESKTOP + mpv `--wid`, atau alternatif) dan mengapa.
- Opsi mpv final untuk GM45 (vo, gpu-context, hwdec).
- Solusi interaksi Caja dan cara pemulihannya.
- Batas yang diketahui.

## Kasus tepi yang harus diuji
- Window desktop muncul di Alt+Tab/pager (harus tidak).
- Ganti virtual desktop (workspace): wallpaper tetap terlihat di semua.
- Suspend/resume laptop: video lanjut atau dipulihkan.
- Layar tidur/DPMS dan bangun.
- Kompositing marco hidup vs mati.
- Path video berspasi/unicode.
- Video tanpa audio, dan video dengan audio (pastikan tidak bersuara saat `--no-audio`).

## Gate
- [ ] Video berputar sebagai wallpaper **10 menit tanpa crash** (tanpa artefak visual)
- [ ] Ikon desktop tetap berfungsi, atau batasan terdokumentasi jelas dan disetujui lewat BLOKIR
- [ ] `kill -9` pada program → tidak ada proses mpv/window yatim
- [ ] `kill -9` pada mpv → pulih otomatis < 3 detik
- [ ] Tabel pengukuran terisi di `docs/ARCH.md`
- [ ] Putusan teknis (1.9) tertulis
- [ ] Jika salah satu di atas mustahil: BLOKIR dikirim dan fase **tidak** ditutup

## Artefak
`crates/vallentheme-render/examples/*`, catatan di `docs/ARCH.md`, skrip uji (`scripts/spike-*.sh`).

## Jangan lakukan
Membangun D-Bus, UI, atau database. Mengubah setting desktop pengguna tanpa mencatat nilai awal dan memulihkannya.
