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
