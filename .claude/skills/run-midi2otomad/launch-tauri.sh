#!/usr/bin/env bash
# Launch the REAL compiled Tauri app headless under Xvfb + WebKitGTK and
# screenshot the rendered window. A *debug* Tauri build loads the dev URL
# (http://localhost:1420), so this also starts `trunk serve` (exactly what
# `cargo tauri dev` does via beforeDevCommand) unless one is already up.
#
# Exits non-zero (so it works as a smoke test) when the dev server never
# becomes reachable, the app dies before a window appears, or the screenshot
# comes back blank. The trunk server it starts is always torn down on exit.
#
# Usage: launch-tauri.sh [out.png] [seconds-before-shot]
# Prereqs (apt): xvfb dbus-x11 imagemagick xdotool + libwebkit2gtk-4.1 (+ trunk)
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN="${BIN:-$ROOT/target/debug/midi2otomad}"
OUT="${1:-/tmp/m2o-shots/tauri.png}"
WAIT="${2:-10}"
TRUNK="${TRUNK:-$HOME/.cargo/bin/trunk}"
mkdir -p "$(dirname "$OUT")"
LOG="$(dirname "$OUT")/tauri.log"
SERVE_LOG="$(dirname "$OUT")/trunk-serve.log"
RUNTIME_DIR="$(mktemp -d)"

serve_pid=""
cleanup() {
  [ -n "$serve_pid" ] && kill "$serve_pid" 2>/dev/null
  rm -rf "$RUNTIME_DIR"
}
trap cleanup EXIT

port_up() { [ "$(curl -s -o /dev/null -w '%{http_code}' http://localhost:1420/ 2>/dev/null)" = "200" ]; }

[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build -p midi2otomad" >&2; exit 1; }
[ -d "$ROOT/ui/dist" ] || { echo "missing ui/dist — run: (cd ui && trunk build)" >&2; exit 1; }

# Start the dev server ourselves unless one is already serving. Track its PID so
# cleanup() can kill it without depending on fuser/lsof, and fail fast if it
# never serves the UI (otherwise the WebView loads an unreachable dev URL).
# The built UI references a content-hashed bundle; use it to tell whether a
# server already on :1420 is THIS checkout's app (a stale/foreign one would let
# the smoke "pass" without exercising the current UI). We only start our own on
# a free port, so a server we launch is this checkout by construction.
bundle="$(grep -oE 'midi2otomad[A-Za-z0-9._-]+\.js' "$ROOT/ui/dist/index.html" | head -1)"
serves_checkout() { [ -n "$bundle" ] && curl -s http://localhost:1420/ 2>/dev/null | grep -q "$bundle"; }

if port_up; then
  serves_checkout || {
    echo "port 1420 is busy but not serving this checkout's UI (expected $bundle); free it: fuser -k 1420/tcp" >&2
    exit 1
  }
else
  ( cd "$ROOT/ui" && exec "$TRUNK" serve --port 1420 --address 127.0.0.1 >"$SERVE_LOG" 2>&1 ) &
  serve_pid=$!
  for _ in $(seq 1 60); do
    port_up && break
    kill -0 "$serve_pid" 2>/dev/null || { echo "trunk serve died — see $SERVE_LOG" >&2; exit 1; }
    sleep 1
  done
  port_up || { echo "dev server not reachable on :1420 after 60s — see $SERVE_LOG" >&2; exit 1; }
fi

export XDG_RUNTIME_DIR="$RUNTIME_DIR"
# Make WebKitGTK paint inside a headless X server (no GPU, no DMABUF/compositor).
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export GDK_BACKEND=x11
export NO_AT_BRIDGE=1

# Inner script gets host values as positional args ($1..$4) — no quote-juggling.
# It returns non-zero if the app exits before a window appears (exit 3) or no
# window ever shows (exit 4), so a crash can't masquerade as a passing smoke.
xvfb-run -a -s "-screen 0 1600x1000x24" dbus-run-session -- bash -c '
  bin="$1"; out="$2"; waitsec="$3"; log="$4"
  "$bin" >"$log" 2>&1 &
  app=$!
  ready=""
  for _ in $(seq 1 30); do
    if xdotool search --name midi2otomad >/dev/null 2>&1; then ready=1; break; fi
    kill -0 "$app" 2>/dev/null || break
    sleep 0.5
  done
  if [ -z "$ready" ] && ! kill -0 "$app" 2>/dev/null; then
    echo "app exited early before a window appeared" >&2; exit 3
  fi
  sleep "$waitsec"
  wid=$(xdotool search --name midi2otomad 2>/dev/null | tail -1)
  if [ -n "$wid" ]; then
    xdotool windowactivate "$wid" 2>/dev/null
    import -window "$wid" "$out" 2>/dev/null || import -window root "$out"
  else
    import -window root "$out"
  fi
  kill "$app" 2>/dev/null; wait "$app" 2>/dev/null
  [ -n "$wid" ] || { echo "no app window ever appeared" >&2; exit 4; }
' _ "$BIN" "$OUT" "$WAIT" "$LOG"
rc=$?

echo "--- app log (tail) ---"; grep -vE 'ALSA lib|snd_' "$LOG" 2>/dev/null | tail -8
[ "$rc" -eq 0 ] || { echo "launch failed (rc=$rc): the app did not render — see $LOG" >&2; exit "$rc"; }

# A blank Xvfb capture is a few KB; the real studio UI is ~100 KB+.
size=$(stat -c%s "$OUT" 2>/dev/null || echo 0)
[ "$size" -ge 8000 ] || { echo "screenshot looks blank (${size} B) — WebView didn't paint; see $LOG" >&2; exit 5; }
echo "screenshot -> $OUT (${size} B)"
