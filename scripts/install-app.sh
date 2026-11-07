#!/usr/bin/env bash
# Pasang vallentheme untuk pengguna saat ini: ikon tema, entri menu (launcher),
# dan jalur pintas di terminal.
#
# Pakai biner rilis bila ada, kalau tidak biner debug.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/vallentheme-gui"
[ -x "$BIN" ] || BIN="$ROOT/target/debug/vallentheme-gui"
[ -x "$BIN" ] || { echo "biner belum dibangun: cargo build --release" >&2; exit 1; }

ICON_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
LAUNCH_DIR="$HOME/.local/bin"

for entry in "$ROOT"/data/icons/hicolor/*/apps/vallentheme.png; do
    size="$(basename "$(dirname "$(dirname "$entry")")")"
    mkdir -p "$ICON_ROOT/$size/apps"
    install -m 0644 "$entry" "$ICON_ROOT/$size/apps/vallentheme.png"
done

mkdir -p "$APP_DIR"
sed "s|@BIN@|$BIN|" "$ROOT/data/id.vallentheme.App.desktop" > "$APP_DIR/id.vallentheme.App.desktop"
chmod 0644 "$APP_DIR/id.vallentheme.App.desktop"

mkdir -p "$LAUNCH_DIR"
cat > "$LAUNCH_DIR/vallentheme" <<EOF
#!/bin/sh
exec "$BIN" "\$@"
EOF
chmod 0755 "$LAUNCH_DIR/vallentheme"

command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$ICON_ROOT" || true

echo "terpasang:"
echo "  biner   : $BIN"
echo "  ikon    : $ICON_ROOT/*/apps/vallentheme.png"
echo "  launcher: $APP_DIR/id.vallentheme.App.desktop"
echo "  perintah: $LAUNCH_DIR/vallentheme"
