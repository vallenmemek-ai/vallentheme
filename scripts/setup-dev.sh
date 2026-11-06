#!/usr/bin/env bash
# Cetak perintah pemasangan dependensi sistem. Tidak menjalankan sudo sendiri.
set -euo pipefail

cat <<'PERINTAH'
Jalankan perintah berikut (butuh sudo):

  sudo apt update
  sudo apt install -y build-essential pkg-config git \
      libgtk-4-dev libadwaita-1-dev libsqlite3-dev \
      mpv i965-va-driver vainfo ffmpeg sysstat

Verifikasi setelahnya:

  pkg-config --modversion gtk4 libadwaita-1 sqlite3
  mpv --version | head -1
  ffmpeg -version | head -1
  rustc --version && cargo --version
  vainfo 2>&1 | head -10

Catat hasilnya di docs/ARCH.md.
PERINTAH
