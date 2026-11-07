#!/bin/bash
set -euo pipefail

BINDIR="${1:-$HOME/.local/bin}"
SERVICE_FILE="data/id.vallentheme.Daemon.service"
DBUS_DIR="$HOME/.local/share/dbus-1/services"

mkdir -p "$DBUS_DIR"

sed "s|@BINDIR@|$BINDIR|g" "$SERVICE_FILE" > "$DBUS_DIR/id.vallentheme.Daemon.service"

echo "D-Bus service installed: $DBUS_DIR/id.vallentheme.Daemon.service"
echo "Daemon akan auto-start saat GUI panggil Apply"
