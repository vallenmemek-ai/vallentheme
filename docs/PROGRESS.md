# PROGRESS

Laporan per fase. Format mengikuti `README` paket rencana.

---

## Fase 0 — Persiapan & fondasi repo (2026-11-06)

Status: SELESAI

Gate:
- [x] `cargo build --workspace` sukses dari nol (`cargo clean` dulu)
- [x] `scripts/check.sh` hijau
- [x] `pkg-config --modversion gtk4 libadwaita-1` menampilkan versi; libadwaita ≥ 1.5 (4.18.6 / 1.7.6)
- [x] Menjalankan tiap binary menulis log ke direktori state dan keluar bersih (kode 0)
- [x] Loader config lolos semua kasus tepi di 0.7
- [x] `docs/ARCH.md` memuat versi alat dan spek mesin

Yang dibuat:
- Workspace Cargo 4 crate + lint ketat + profil build ringan.
- `core`: `error` (kode `VT-*` + pesan Indonesia), `paths` (XDG, izin 0700),
  `config` (TOML toleran, validasi, simpan atomik, cadangan berkas rusak),
  `logging` (stderr + berkas harian, pangkas 5 berkas).
- Binary kerangka `vallentheme-daemon` dan `vallentheme-gui` (belum ada GUI/DB).
- `scripts/check.sh`, `scripts/setup-dev.sh`, `scripts/soak.sh` (kerangka).
- `docs/ARCH.md`, `docs/PROGRESS.md`, `docs/IDEAS.md`, `LICENSE` (TBD).

Keputusan teknis:
- `README.md` lama (paket rencana) dipindahkan ke `docs/RENCANA-PAKET.md`;
  `README.md` di root diganti README proyek agar sesuai Fase 0.0.2.
- `directories` dipakai untuk seluruh path XDG, termasuk `state_dir()`.
- `XDG_RUNTIME_DIR` tidak tersedia → fallback ke `<state>/runtime` dengan
  peringatan log (bukan crash).

Bug/utang teknis:
- Belum ada dependensi GUI/DB (sesuai larangan fase ini).
- Sesi pengembangan headless; uji visual GUI nanti memakai `Xvfb`.

Hasil ukur:
- Build dari nol (`cargo clean` lalu `cargo build --workspace`): **112 detik**, 2 core.
- Ukuran biner debug: **23 MB** per binary.
- Test: **13 lulus, 0 gagal**.
- Pohon dependensi normal: **68 paket**. Duplikat hanya `syn` v2 + v3 (proc-macro,
  hanya saat build, tidak masuk biner rilis) — diterima.
- Binary menulis log ke `~/.local/state/vallentheme/logs/` dan keluar kode 0.
- `config.toml` dibuat otomatis dengan nilai bawaan saat pertama dijalankan.
- Folder runtime `$XDG_RUNTIME_DIR/vallentheme` dibuat dengan izin `0700`.

---

## Fase 1 — Spike render X11 (2026-11-06)

Status: SEBAGIAN — semua butir terukur lulus, **kecuali** butir ikon desktop yang
menunggu keputusan pemilik proyek (BLOKIR di bawah).

Gate:
- [x] Video berputar sebagai wallpaper **10 menit tanpa crash** (620 detik, 0 frame drop, 0 restart, RSS stabil 173 MB)
- [ ] Ikon desktop tetap berfungsi — **BLOKIR**, lihat bagian di bawah
- [x] `kill -9` pada program → tidak ada proses mpv/window yatim
- [x] `kill -9` pada mpv → pulih otomatis dalam **2543 ms** (< 3 detik)
- [x] Tabel pengukuran terisi di `docs/ARCH.md` (7 baris, data mentah `docs/ukur-fase1.csv`)
- [x] Putusan teknis (1.9) tertulis di `docs/ARCH.md` bagian 3

Yang dibuat:
- `render::x11` — koneksi X11, daftar monitor (RandR), pembuatan window DESKTOP,
  pelukisan, penurunan/penaikan tumpukan, pemantauan properti root.
- `render::process` — menjalankan anak tanpa shell, `PR_SET_PDEATHSIG`,
  grup proses sendiri, penghentian sopan lalu `SIGKILL`, `setpriority`.
- `render::error` — kode `VT-X11-001..004`, `VT-MPV-001`.
- Contoh spike: `spike_window`, `spike_mpv`, `spike_focus`.
- Skrip: `scripts/spike-matrix.sh`, `scripts/spike-resilience.sh`.

Keputusan teknis:
- Window DESKTOP + `mpv --wid`, `--vo=gpu`, `--hwdec=auto-safe`.
- `background_pixel` tidak cukup di bawah compositor; window wajib dilukis
  sendiri dan dilukis ulang saat `Expose`.
- `--vo=gpu` dipilih: 4K 77,5% CPU vs `x11` 91,5% CPU.
- hwdec tidak bisa diandalkan di mesin ini (GM45 hanya MPEG-2).

Bug/utang teknis:
- Deteksi **maximize** belum jalan lewat properti root; perlu berlangganan
  `_NET_WM_STATE` pada window klien (dikerjakan di Fase 5/7).
- Belum ada penanganan `RRScreenChangeNotify` (Fase 5).
- Cermin ikon Caja (Composite/Damage) belum dibuat (usul v0.2).

Hasil ukur: lihat `docs/ARCH.md` bagian 3.6 dan `docs/ukur-fase1.csv`.

## BLOKIR — Fase 1

Konteks: membuktikan video bisa jadi wallpaper di MATE + X11. Wallpaper sudah
berjalan, tetapi bertabrakan dengan ikon desktop Caja.

Masalah: window desktop Caja berkedalaman 24-bit (opaque), jadi selalu menutupi
window wallpaper. `draw-background false` (opsi b di `panduan/06`) sudah diuji
dan **tidak** membuatnya transparan. Akibatnya hanya satu yang bisa terlihat:
wallpaper atau ikon desktop.

Yang sudah dicoba:
1. Window `BELOW` biasa → wallpaper 0 piksel terlihat.
2. Naikkan window di atas window Caja → wallpaper terlihat (214.548 piksel),
   ikon tertutup.
3. `gsettings set org.mate.background draw-background false` + restart Caja →
   Caja tetap melukis warna solid.
4. `override_redirect` — tidak mengubah hasil (masalahnya tumpukan, bukan WM).

Opsi:
- **A)** v0.1: wallpaper menang, ikon desktop disembunyikan, tampilkan
  pemberitahuan sekali. Sederhana, stabil, selesai cepat. (Rekomendasi agent)
- **B)** Tiru Fresco: latar Caja diberi warna kunci `#010101`, lalu salin isi
  window Caja (ikon) ke atas wallpaper memakai X Composite + Damage
  (chroma-key). Ikon tetap terlihat, tetapi menambah ~1–2 hari kerja dan
  bergantung pada ekstensi Composite/Damage.
- **C)** v0.1 hanya mengaktifkan wallpaper video bila `show-desktop-icons`
  bernilai `false` (kondisi mesin pengembang); bila ikon menyala, tampilkan
  penjelasan dan tawarkan mematikannya.

Rekomendasi agent: **A** untuk v0.1, catat **B** di `docs/IDEAS.md` sebagai
target v0.2.
