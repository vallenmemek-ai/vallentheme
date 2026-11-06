#!/usr/bin/env bash
# Uji ketahanan Fase 1.7: PDEATHSIG dan pemulihan otomatis.
# Pemakaian: scripts/spike-resilience.sh
set -uo pipefail
cd "$(dirname "$0")/.."

BIN=./target/debug/examples/spike_mpv
export DISPLAY=:0

lapor() { printf '\n=== %s ===\n' "$1"; }

lapor "A. kill -9 pada induk harus membunuh mpv (PR_SET_PDEATHSIG)"
"$BIN" /tmp/uji-1080p.mp4 60 gpu no >/tmp/res-a.log 2>&1 &
INDUK=$!
sleep 8
MPV=$(pgrep -n -x mpv)
echo "induk=$INDUK mpv=$MPV"
kill -9 "$INDUK"
sleep 2
if pgrep -x mpv >/dev/null; then
    echo "GAGAL: mpv masih hidup setelah induk dibunuh"
    pkill -x mpv
else
    echo "LULUS: tidak ada mpv yatim"
fi
wait "$INDUK" 2>/dev/null

lapor "B. kill -9 pada mpv harus dipulihkan < 3 detik"
"$BIN" /tmp/uji-1080p.mp4 45 gpu no >/tmp/res-b.log 2>&1 &
INDUK=$!
sleep 8
MPV=$(pgrep -n -x mpv)
echo "membunuh mpv $MPV"
T0=$(date +%s%N)
kill -9 "$MPV"
BARU=""
for _ in $(seq 1 60); do
    sleep 0.1
    BARU=$(pgrep -n -x mpv || true)
    [ -n "$BARU" ] && [ "$BARU" != "$MPV" ] && break
done
T1=$(date +%s%N)
if [ -n "$BARU" ] && [ "$BARU" != "$MPV" ]; then
    echo "LULUS: mpv baru=$BARU dalam $(( (T1 - T0) / 1000000 )) ms"
else
    echo "GAGAL: mpv tidak dipulihkan"
fi
sleep 3
kill "$INDUK" 2>/dev/null
sleep 2
pkill -x mpv 2>/dev/null
wait "$INDUK" 2>/dev/null

lapor "C. Sisa proses dan window"
echo "proses mpv: $(pgrep -c -x mpv || echo 0)"
echo "window desktop tersisa:"
for w in $(xwininfo -root -children | grep -oE '0x[0-9a-f]+'); do
    xprop -id "$w" WM_CLASS 2>/dev/null | grep -q vallentheme && echo "  $w"
done
echo "(kosong = bersih)"
