#!/usr/bin/env bash
# Launch the REAL compiled Tauri app headless under Xvfb + WebKitGTK and
# screenshot the rendered window. A *debug* Tauri build loads the dev URL
# (http://localhost:1420), so this also starts `trunk serve` (exactly what
# `cargo tauri dev` does via beforeDevCommand) unless one is already up.
#
# Usage: launch-tauri.sh [out.png] [seconds-before-shot]
# Prereqs (apt): xvfb dbus-x11 imagemagick xdotool + libwebkit2gtk-4.1 (+ trunk)
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN="$ROOT/target/debug/midi2otomad"
OUT="${1:-/tmp/m2o-shots/tauri.png}"
WAIT="${2:-10}"
TRUNK="${TRUNK:-$HOME/.cargo/bin/trunk}"
mkdir -p "$(dirname "$OUT")"
LOG="$(dirname "$OUT")/tauri.log"

[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build -p midi2otomad" >&2; exit 1; }
[ -d "$ROOT/ui/dist" ] || { echo "missing ui/dist — run: (cd ui && trunk build)" >&2; exit 1; }

started_serve=""
if [ "$(curl -s -o /dev/null -w '%{http_code}' http://localhost:1420/ 2>/dev/null)" != "200" ]; then
  ( cd "$ROOT/ui" && "$TRUNK" serve --port 1420 --address 127.0.0.1 >"$(dirname "$OUT")/trunk-serve.log" 2>&1 ) &
  started_serve=1
  for i in $(seq 1 60); do
    [ "$(curl -s -o /dev/null -w '%{http_code}' http://localhost:1420/ 2>/dev/null)" = "200" ] && break
    sleep 1
  done
fi

export XDG_RUNTIME_DIR="$(mktemp -d)"
# Make WebKitGTK paint inside a headless X server (no GPU, no DMABUF/compositor).
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export GDK_BACKEND=x11
export NO_AT_BRIDGE=1

xvfb-run -a -s "-screen 0 1600x1000x24" dbus-run-session -- bash -c '
  "'"$BIN"'" >"'"$LOG"'" 2>&1 &
  app=$!
  for i in $(seq 1 30); do
    xdotool search --name midi2otomad >/dev/null 2>&1 && break
    kill -0 $app 2>/dev/null || { echo "app exited early"; break; }
    sleep 0.5
  done
  sleep "'"$WAIT"'"
  wid=$(xdotool search --name midi2otomad 2>/dev/null | tail -1)
  if [ -n "$wid" ]; then
    xdotool windowactivate "$wid" 2>/dev/null
    import -window "$wid" "'"$OUT"'" 2>/dev/null || import -window root "'"$OUT"'"
  else
    import -window root "'"$OUT"'"
  fi
  kill $app 2>/dev/null; wait $app 2>/dev/null
'
[ -n "$started_serve" ] && fuser -k 1420/tcp 2>/dev/null
echo "screenshot -> $OUT"; ls -la "$OUT" 2>/dev/null
echo "--- app log (tail) ---"; grep -vE 'ALSA lib|snd_' "$LOG" 2>/dev/null | tail -8
