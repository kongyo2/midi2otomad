import http from "node:http";
import { readFileSync, existsSync, statSync, mkdirSync } from "node:fs";
import { join, resolve, sep, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const SKILL = dirname(fileURLToPath(import.meta.url));
const ROOT = join(SKILL, "..", "..", "..");
const DIST = join(ROOT, "ui", "dist");
const SHOTS = process.env.M2O_SHOTS || "/tmp/m2o-shots";
mkdirSync(SHOTS, { recursive: true });

const pageErrors = [];

const MIME = {
  ".html": "text/html", ".js": "text/javascript", ".css": "text/css",
  ".wasm": "application/wasm", ".json": "application/json",
};

function serveDist() {
  if (!existsSync(DIST)) {
    console.error(`missing ${DIST} — run: (cd ui && trunk build)`);
    process.exit(1);
  }
  const server = http.createServer((req, res) => {
    let p = decodeURIComponent(req.url.split("?")[0]);
    if (p === "/" || p === "") p = "/index.html";
    const file = resolve(DIST, "." + p);
    if (!file.startsWith(DIST + sep)) { res.writeHead(403); res.end(); return; }
    if (!existsSync(file) || !statSync(file).isFile()) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": MIME[extname(file)] || "application/octet-stream" });
    res.end(readFileSync(file));
  });
  return new Promise((r) => server.listen(0, "127.0.0.1", () => r(server)));
}

const peaks = Array.from({ length: 600 }, (_, i) =>
  Math.abs(Math.sin(i / 18)) * Math.max(0.12, 1 - i / 600));
const toneDto = { id: "sample-1", name: "tone", fileName: "tone.wav", durationSec: 0.4, peaks };
const melodyProject = {
  version: 1, name: "melody.mid", bpm: 120, ppq: 480, sampleRate: 48000, masterGain: 1.0,
  tracks: [{
    id: "track-1", name: "Track 1", midiIndex: 0,
    notes: [
      { pitch: 60, startSec: 0.0, durationSec: 0.5, velocity: 100 },
      { pitch: 64, startSec: 0.5, durationSec: 0.5, velocity: 100 },
      { pitch: 67, startSec: 1.0, durationSec: 0.5, velocity: 100 },
    ],
  }],
};

const SHIM = `
window.__m2o = { handlers: {}, calls: [], unexpected: [] };
window.__TAURI__ = {
  core: {
    invoke: async (cmd, args) => {
      if (cmd !== 'status') window.__m2o.calls.push(cmd);
      switch (cmd) {
        case 'status': return { playing: false, position: 0, duration: 0, level: 0 };
        case 'set_mix': return { durationSec: 1.5, peak: 0.6 };
        case 'open_midi': return { project: ${JSON.stringify(melodyProject)}, trackCount: 1, noteCount: 3 };
        case 'open_audio': return [${JSON.stringify(toneDto)}];
        case 'ingest_paths': return { import: { project: ${JSON.stringify(melodyProject)}, trackCount: 1, noteCount: 3 }, samples: [${JSON.stringify(toneDto)}] };
        case 'export': return { path: '/tmp/otomad.wav', bytes: 288044, durationSec: 1.5 };
        case 'play': case 'pause': case 'stop':
        case 'seek': case 'remove_sample': case 'preview_sample':
          return null;
        default:
          window.__m2o.unexpected.push(cmd);
          return null;
      }
    },
  },
  event: {
    listen: async (event, handler) => { window.__m2o.handlers[event] = handler; return () => {}; },
  },
};
`;

async function open() {
  const server = await serveDist();
  const port = server.address().port;
  const browser = await chromium.launch({ args: ["--no-sandbox"] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.on("pageerror", (e) => { pageErrors.push(e.message); console.error("PAGE ERROR:", e.message); });
  await page.addInitScript(SHIM);
  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForSelector(".studio", { timeout: 15000 });
  await page.waitForTimeout(400);
  return { browser, server, page };
}
const shot = (page, f) => page.screenshot({ path: f.includes("/") ? f : join(SHOTS, f) });
const clickText = (page, t) => page.getByText(t, { exact: false }).first().click();

const cmd = process.argv[2] || "shot";
const { browser, server, page } = await open();
try {
  if (cmd === "shot") {
    const f = process.argv[3] || "ui.png";
    await shot(page, f);
    console.log("screenshot ->", f.includes("/") ? f : join(SHOTS, f));
  } else if (cmd === "flow") {
    const dir = process.argv[3] || SHOTS;
    mkdirSync(dir, { recursive: true });
    await shot(page, join(dir, "01-initial.png"));
    await clickText(page, "MIDI を開く"); await page.waitForTimeout(500);
    await shot(page, join(dir, "02-midi-loaded.png"));
    await clickText(page, "追加"); await page.waitForTimeout(500);
    await shot(page, join(dir, "03-sample-added.png"));
    await page.locator(".panel", { hasText: "マスターリバーブ" }).getByRole("checkbox").check();
    await page.waitForTimeout(300);
    await shot(page, join(dir, "04-reverb-on.png"));
    console.log("calls:", JSON.stringify(await page.evaluate(() => window.__m2o.calls)));
    console.log("screenshots ->", dir);
  } else if (cmd === "eval") {
    const out = await page.evaluate(process.argv[3] || "1+1");
    console.log(JSON.stringify(out));
  } else if (cmd === "repl") {
    const rl = (await import("node:readline")).createInterface({ input: process.stdin });
    console.log("READY");
    for await (const line of rl) {
      const [c, ...rest] = line.trim().split(" ");
      const arg = rest.join(" ");
      try {
        if (c === "quit" || c === "exit") break;
        else if (c === "shot") { await shot(page, arg || "ui.png"); console.log("OK", arg || "ui.png"); }
        else if (c === "click") { await clickText(page, arg); console.log("OK click", arg); }
        else if (c === "sel") { await page.locator(arg).first().click(); console.log("OK sel", arg); }
        else if (c === "text") { console.log(await page.locator(arg).first().innerText()); }
        else if (c === "eval") { console.log(JSON.stringify(await page.evaluate(arg))); }
        else if (c === "wait") { await page.waitForTimeout(parseInt(arg || "200")); console.log("OK"); }
        else if (c) console.log("?", c);
      } catch (e) { console.log("ERR", e.message); }
    }
  } else {
    console.error("unknown cmd:", cmd);
    process.exitCode = 2;
  }
} finally {
  let unexpected = [];
  try { unexpected = await page.evaluate(() => window.__m2o?.unexpected ?? []); } catch {}
  await browser.close();
  server.close();
  if (pageErrors.length) {
    console.error(`FAILED: ${pageErrors.length} uncaught page error(s)`);
    if (!process.exitCode) process.exitCode = 1;
  }
  if (unexpected.length) {
    console.error(`FAILED: UI invoked unknown backend command(s): ${unexpected.join(", ")}`);
    if (!process.exitCode) process.exitCode = 1;
  }
}
