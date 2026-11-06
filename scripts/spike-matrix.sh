#!/usr/bin/env bash
# Jalankan matriks pengukuran Fase 1 dan tulis hasil ke CSV.
set -uo pipefail
cd "$(dirname "$0")/.."

BIN=./target/debug/examples/spike_mpv
DETIK="${1:-45}"
OUT="docs/ukur-fase1.csv"
echo "video,hwdec,diminta,vo,cpu_persen,rss_mb,drop" > "$OUT"

jalankan() {
    local video="$1" hwdec="$2" vo="$3" label="$4"
    local log
    log=$(mktemp)
    DISPLAY=:0 "$BIN" "$video" "$DETIK" "$vo" "$hwdec" > "$log" 2>&1
    local cpu rss drop aktual
    cpu=$(grep -oP 'CPU rata-rata: \K[0-9.]+' "$log" || echo "?")
    rss=$(grep -oP 'RSS mpv: \K[0-9]+' "$log" || echo "?")
    drop=$(grep -oP 'frame drop sesi ini: \K[0-9]+' "$log" || echo "?")
    aktual=$(grep -oP 'hwdec-current=\K[^ ]+' "$log" | tail -1 || echo "?")
    echo "$label,$aktual,$hwdec,$vo,$cpu,$rss,$drop" >> "$OUT"
    echo "[$label] minta=$hwdec aktual=$aktual cpu=${cpu}% rss=${rss}MB drop=${drop}"
    rm -f "$log"
    sleep 2
}

jalankan /tmp/uji-1080p.mp4 no          gpu "1080p H.264 24fps"
jalankan /tmp/uji-1080p.mp4 vaapi       gpu "1080p H.264 24fps"
jalankan /tmp/uji-1080p.mp4 auto-safe   gpu "1080p H.264 24fps"
jalankan /tmp/uji-4k.mp4    auto-safe   gpu "4K H.264 24fps"
jalankan /tmp/uji-4k.mp4    no          x11 "4K H.264 24fps"
jalankan /tmp/uji-vp9.webm  auto-safe   gpu "1080p VP9 30fps"
jalankan /tmp/uji-foto.jpg  auto-safe   gpu "foto 4K"

echo "=== SELESAI ==="
cat "$OUT"
