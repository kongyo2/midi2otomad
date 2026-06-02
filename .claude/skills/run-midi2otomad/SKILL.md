---
name: run-midi2otomad
description: Build, run, and drive the midi2otomad Tauri desktop app. Use when asked to start/run midi2otomad, build it, take a screenshot of its GUI, drive its UI, or run its tests. It is a Tauri 2 app with a Leptos (Rust/WASM) frontend and a Rust audio backend.
---

midi2otomad is a **Tauri 2** desktop app (MIDI 音MAD studio): a Leptos/Rust→WASM
frontend in a WebKitGTK webview, a Rust backend (`src-tauri`, cpal playback +
native file dialogs), and a pure-Rust DSP/MIDI/codec engine (`core`). Three
handles, pick by what you changed:

- **`core/` change (DSP, MIDI, audio, media) — most PRs** → `cargo test`. The
  suite *is* the end-to-end smoke (import MIDI → mix → encode/decode WAV/MP3).
- **`ui/` change (Leptos components)** → `.claude/skills/run-midi2otomad/driver.mjs`
  — Playwright drives the real built frontend headless (real DOM, real clicks),
  with the Tauri backend mocked. Fast.
- **Whole-app / `src-tauri` / "does it still render"** → `.claude/skills/run-midi2otomad/launch-tauri.sh`
  — launches the **actual compiled binary** under Xvfb+WebKitGTK and screenshots it.

All paths below are relative to the repo root (the unit).

## Prerequisites

Driver/screenshot tooling (Ubuntu). `xvfb`/`Xvfb` were already present in this
container; the rest is what actually got installed:

```bash
sudo apt-get update
sudo apt-get install -y xvfb dbus-x11 imagemagick xdotool psmisc
```

Building the Tauri backend needs the WebKitGTK dev libs. They were **already
present here** (`pkg-config --exists webkit2gtk-4.1` succeeds); on a clean
machine install the CI set:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libasound2-dev
```

## Setup

```bash
rustup target add wasm32-unknown-unknown

# trunk (WASM bundler) — prebuilt binary into ~/.cargo/bin:
tag=$(curl -sI -A "claude-code/1.0" https://github.com/trunk-rs/trunk/releases/latest \
      | tr -d '\r' | awk -F'tag/' '/^location:/{print $2}')
curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/${tag}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
      | tar xz -C ~/.cargo/bin trunk
trunk --version           # alternative: cargo install --locked trunk

# Chromium for the UI driver. Playwright is preinstalled globally here
# (npm root -g); if missing: npm i -g playwright && npx playwright install chromium
npx playwright install chromium
```

## Build

```bash
(cd ui && trunk build)          # -> ui/dist (the WASM bundle; ~1 min cold)
cargo build -p midi2otomad      # -> target/debug/midi2otomad (~1.5 min cold)
```

## Run (agent path)

### Screenshot the real app (`launch-tauri.sh`)

Launches `target/debug/midi2otomad` headless under Xvfb+WebKitGTK and grabs the
window. A *debug* build loads the dev URL, so the script starts `trunk serve`
on 1420 itself (what `cargo tauri dev` does) and tears it down after. It **exits
non-zero** if the dev server never comes up (or `:1420` is already serving a
different build), the app dies before a window appears, or the screenshot is
blank — so it doubles as an integration smoke. A `trunk serve` you started from
this checkout is detected and reused (and left running).

```bash
bash .claude/skills/run-midi2otomad/launch-tauri.sh /tmp/m2o-shots/tauri.png 10
# -> /tmp/m2o-shots/tauri.png  (full studio UI; ~107 KB. log: /tmp/m2o-shots/tauri.log)
```

### Drive the UI headless (`driver.mjs`)

Serves the built `ui/dist`, opens it in Playwright Chromium with a
`window.__TAURI__` shim, and clicks the real Leptos DOM. Export `NODE_PATH` once
so Node finds the global Playwright:

```bash
export NODE_PATH="$(npm root -g)"
node .claude/skills/run-midi2otomad/driver.mjs shot ui.png
node .claude/skills/run-midi2otomad/driver.mjs flow /tmp/m2o-shots/flow
node .claude/skills/run-midi2otomad/driver.mjs eval "document.querySelector('.topbar__title').textContent"
```

`flow` clicks **MIDI を開く** → **追加** → reverb toggle and writes
`01-initial.png` … `04-reverb-on.png` (melody loads as 3 notes C4/E4/G4, the
`tone` sample appears with a waveform and auto-assigns to the track).

REPL over stdin for ad-hoc poking (one command per line):

```bash
printf 'click MIDI を開く\nwait 500\ntext .topbar__title\nshot after.png\nquit\n' \
  | NODE_PATH="$(npm root -g)" node .claude/skills/run-midi2otomad/driver.mjs repl
```

| driver.mjs cmd | what it does |
|---|---|
| `shot [f.png]` | initial render → screenshot |
| `flow [dir]` | load MIDI + add sample + reverb-on → 4 screenshots |
| `eval '<js>'` | run JS in the page, print JSON result |
| `repl` | stdin: `shot f` · `click <text>` · `sel <css>` · `text <css>` · `eval <js>` · `wait <ms>` · `quit` |

`shot`/`flow` exit non-zero on an uncaught page error (e.g. a WASM panic via
`console_error_panic_hook`), an unknown driver command, or if the UI invokes a
backend command the mock doesn't know (broken/renamed wiring) — so they work as
UI smoke checks.

Need real `.mid`/`.wav` files (for the real app's drag-drop / ingest, which read
from disk — the driver's backend is mocked, so it doesn't use them):

```bash
node .claude/skills/run-midi2otomad/make-fixtures.mjs /tmp/m2o-fix   # -> melody.mid, tone.wav
```

Artifacts → `/tmp/m2o-shots/` (override with `M2O_SHOTS`). App log → `/tmp/m2o-shots/tauri.log`.

## Test

```bash
cargo test -p midi2otomad-core -p midi2otomad-ui -p midi2otomad
```

Expect **215 passing** (core 198 across lib + mix_pipeline/midi_to_audio/media_roundtrip,
ui 9, backend 8). `cargo test -p midi2otomad-core` alone is the fast inner loop
for engine work. Match CI's gates with `cargo fmt --all --check`,
`cargo clippy -p midi2otomad-core --all-targets -- -D warnings`, and
`cargo clippy -p midi2otomad-ui --target wasm32-unknown-unknown -- -D warnings`.

## Run (human path)

`cargo tauri dev` is the upstream one-command launcher (needs `cargo install
tauri-cli`); it opens a window and blocks, so it's useless headless — use
`launch-tauri.sh` instead.

## Gotchas

- **Debug build → blank "Could not connect to localhost: Connection refused"
  webview.** A debug Tauri build loads `devUrl` (`http://localhost:1420`), not
  the embedded `ui/dist`. Something must serve 1420. `launch-tauri.sh` starts
  `trunk serve` itself; if you launch the binary by hand, start it first.
- **WebKitGTK paints nothing in Xvfb** without forcing software rendering. The
  launcher exports `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`, `LIBGL_ALWAYS_SOFTWARE=1` — drop these and
  the screenshot is a solid `#16161c` rectangle.
- **No audio device in the container.** cpal init fails and the app logs
  `オーディオ出力を初期化できませんでした（無音で続行）` then runs silently — so
  `status().playing` never flips and play/preview are headless no-ops. ALSA
  dumps errors to stderr; filter with `grep -vE 'ALSA lib|snd_'`.
- **Never `pkill -f 'trunk serve'`.** The pattern matches your own shell's
  command line and kills the session (exit 144). Use `fuser -k 1420/tcp`.
- **`driver.mjs` mocks the backend.** It exercises the frontend only — native
  file dialogs (`open_midi`/`open_audio`/`export`), cpal, and real mixing don't
  run. For real decode/mix/encode use `cargo test`; for the real integrated
  window use `launch-tauri.sh`.
- **Playwright is global, not a repo dep** → always `NODE_PATH="$(npm root -g)"`,
  and Chromium launches with `--no-sandbox` (running as root).

## Troubleshooting

- **`trunk: command not found`** → run Setup; also `rustup target add wasm32-unknown-unknown`.
- **`Cannot find package 'playwright'`** → prefix with `NODE_PATH="$(npm root -g)"`; ensure `npx playwright install chromium` ran.
- **`cargo build` fails on a missing `webkit2gtk-4.1`/`gtk+-3.0` `.pc`** → install the build-libs apt line above.
- **Screenshot is a flat dark rectangle** → the WebView didn't paint: the dev server wasn't up (see first gotcha) or the `WEBKIT_DISABLE_*` env vars are missing.
- **`import: command not found`** → `sudo apt-get install -y imagemagick`.
