#!/usr/bin/env bash
# Kerangka soak test: catat RSS dan CPU proses vallentheme tiap menit ke CSV.
# Diisi lengkap di Fase 5. Pemakaian: scripts/soak.sh [menit]
set -euo pipefail

DURASI_MENIT="${1:-60}"
KELUARAN="soak-$(date +%Y%m%d-%H%M%S).csv"

echo "waktu,proses,pid,rss_kb,cpu_persen" > "$KELUARAN"
echo "Menulis ke $KELUARAN selama $DURASI_MENIT menit. Ctrl+C untuk berhenti."

for ((i = 0; i < DURASI_MENIT; i++)); do
    for nama in vallentheme-daemon vallentheme-gui mpv; do
        while read -r pid; do
            [ -z "$pid" ] && continue
            rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo 0)
            cpu=$(ps -o %cpu= -p "$pid" 2>/dev/null | tr -d ' ' || echo 0)
            echo "$(date -Is),$nama,$pid,${rss:-0},${cpu:-0}" >> "$KELUARAN"
        done < <(pgrep -x "$nama" || true)
    done
    sleep 60
done

echo "Selesai. Hasil: $KELUARAN"
