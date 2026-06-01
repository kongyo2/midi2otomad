import { Buffer } from "node:buffer";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const crcTable = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = (c & 1) === 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i += 1) {
    c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuf = Buffer.from(type, "latin1");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([length, typeBuf, data, crc]);
}

function encodePng(rgba, size) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y += 1) {
    const src = y * stride;
    const dst = y * (stride + 1);
    raw[dst] = 0;
    rgba.copy(raw, dst + 1, src, src + stride);
  }
  const idat = deflateSync(raw, { level: 9 });
  return Buffer.concat([signature, pngChunk("IHDR", ihdr), pngChunk("IDAT", idat), pngChunk("IEND", Buffer.alloc(0))]);
}

function buildIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  let offset = 6 + images.length * 16;
  const entries = [];
  const payloads = [];
  for (const { size, png } of images) {
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size;
    entry[1] = size >= 256 ? 0 : size;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(png.length, 8);
    entry.writeUInt32LE(offset, 12);
    entries.push(entry);
    payloads.push(png);
    offset += png.length;
  }
  return Buffer.concat([header, ...entries, ...payloads]);
}

function buildIcns(entries) {
  const parts = [];
  for (const { type, png } of entries) {
    const head = Buffer.alloc(8);
    head.write(type, 0, "latin1");
    head.writeUInt32BE(png.length + 8, 4);
    parts.push(head, png);
  }
  const body = Buffer.concat(parts);
  const header = Buffer.alloc(8);
  header.write("icns", 0, "latin1");
  header.writeUInt32BE(body.length + 8, 4);
  return Buffer.concat([header, body]);
}

function clamp(x, lo, hi) {
  return x < lo ? lo : x > hi ? hi : x;
}

function smoothstep(edge0, edge1, x) {
  const t = clamp((x - edge0) / (edge1 - edge0), 0, 1);
  return t * t * (3 - 2 * t);
}

function sdRoundRect(px, py, cx, cy, halfW, halfH, radius) {
  const dx = Math.abs(px - cx) - (halfW - radius);
  const dy = Math.abs(py - cy) - (halfH - radius);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  const inside = Math.min(Math.max(dx, dy), 0);
  return outside + inside - radius;
}

const VIOLET_LIGHT = [150, 118, 255];
const VIOLET_DEEP = [96, 42, 196];
const MINT = [54, 211, 153];
const MINT_GLOW = [120, 255, 200];
const KEY_WHITE = [250, 250, 253];
const KEY_BLACK = [26, 26, 36];

const BAR_HEIGHTS = [0.3, 0.62, 0.45, 0.85, 1.0, 0.85, 0.45, 0.62, 0.3];
const BAR_X0 = 0.205;
const BAR_X1 = 0.795;
const BAR_Y = 0.315;
const BAR_W = 0.034;
const BAR_HMAX = 0.165;

const KEY_X0 = 0.205;
const KEY_X1 = 0.795;
const KEY_TOP = 0.52;
const KEY_BOTTOM = 0.815;
const WHITE_KEYS = 5;
const KEY_GAP = 0.013;

function buildBars() {
  const bars = [];
  const span = BAR_X1 - BAR_X0 - BAR_W;
  for (let k = 0; k < BAR_HEIGHTS.length; k += 1) {
    const cx = BAR_X0 + BAR_W / 2 + (span * k) / (BAR_HEIGHTS.length - 1);
    const halfH = Math.max(BAR_W / 2, BAR_HEIGHTS[k] * BAR_HMAX);
    bars.push({ cx, cy: BAR_Y, halfW: BAR_W / 2, halfH, radius: BAR_W / 2 });
  }
  return bars;
}

function buildKeys() {
  const keyW = (KEY_X1 - KEY_X0 - (WHITE_KEYS - 1) * KEY_GAP) / WHITE_KEYS;
  const cy = (KEY_TOP + KEY_BOTTOM) / 2;
  const halfH = (KEY_BOTTOM - KEY_TOP) / 2;
  const white = [];
  for (let i = 0; i < WHITE_KEYS; i += 1) {
    const left = KEY_X0 + i * (keyW + KEY_GAP);
    white.push({ cx: left + keyW / 2, cy, halfW: keyW / 2, halfH, radius: 0.02 });
  }
  const blackW = keyW * 0.62;
  const blackBottom = KEY_TOP + (KEY_BOTTOM - KEY_TOP) * 0.58;
  const blackCy = (KEY_TOP + blackBottom) / 2;
  const blackHalfH = (blackBottom - KEY_TOP) / 2;
  const black = [];
  for (const i of [0, 1, 3]) {
    const cx = KEY_X0 + i * (keyW + KEY_GAP) + keyW + KEY_GAP / 2;
    black.push({ cx, cy: blackCy, halfW: blackW / 2, halfH: blackHalfH, radius: 0.014 });
  }
  return { white, black };
}

const BARS = buildBars();
const KEYS = buildKeys();

function coverage(distance, aa) {
  return clamp(0.5 - distance / aa, 0, 1);
}

function composite(out, r, g, b, a) {
  if (a <= 0) {
    return;
  }
  const outA = a + out[3] * (1 - a);
  if (outA <= 0) {
    out[0] = 0;
    out[1] = 0;
    out[2] = 0;
    out[3] = 0;
    return;
  }
  const keep = out[3] * (1 - a);
  out[0] = (r * a + out[0] * keep) / outA;
  out[1] = (g * a + out[1] * keep) / outA;
  out[2] = (b * a + out[2] * keep) / outA;
  out[3] = outA;
}

function scene(x, y, aa, out) {
  out[0] = 0;
  out[1] = 0;
  out[2] = 0;
  out[3] = 0;

  const bgD = sdRoundRect(x, y, 0.5, 0.5, 0.45, 0.45, 0.21);
  const bgCov = coverage(bgD, aa);
  if (bgCov > 0) {
    const t = clamp((x + y) / 2, 0, 1);
    let r = VIOLET_LIGHT[0] + (VIOLET_DEEP[0] - VIOLET_LIGHT[0]) * t;
    let g = VIOLET_LIGHT[1] + (VIOLET_DEEP[1] - VIOLET_LIGHT[1]) * t;
    let b = VIOLET_LIGHT[2] + (VIOLET_DEEP[2] - VIOLET_LIGHT[2]) * t;
    const highlight = Math.pow(clamp(1 - Math.hypot(x - 0.3, y - 0.14) / 0.7, 0, 1), 2) * 0.16;
    r += (255 - r) * highlight;
    g += (255 - g) * highlight;
    b += (255 - b) * highlight;
    const vignette = smoothstep(0.5, 1.02, y) * 0.16;
    composite(out, r * (1 - vignette), g * (1 - vignette), b * (1 - vignette), bgCov);
  }

  let nearestBar = Infinity;
  const barDistances = [];
  for (const bar of BARS) {
    const d = sdRoundRect(x, y, bar.cx, bar.cy, bar.halfW, bar.halfH, bar.radius);
    barDistances.push(d);
    if (d < nearestBar) {
      nearestBar = d;
    }
  }
  const glow = smoothstep(0.05, 0, nearestBar) * 0.24;
  composite(out, MINT_GLOW[0], MINT_GLOW[1], MINT_GLOW[2], glow);
  for (const d of barDistances) {
    composite(out, MINT[0], MINT[1], MINT[2], coverage(d, aa));
  }

  for (const key of KEYS.white) {
    const d = sdRoundRect(x, y, key.cx, key.cy, key.halfW, key.halfH, key.radius);
    composite(out, KEY_WHITE[0], KEY_WHITE[1], KEY_WHITE[2], coverage(d, aa));
  }
  for (const key of KEYS.black) {
    const d = sdRoundRect(x, y, key.cx, key.cy, key.halfW, key.halfH, key.radius);
    composite(out, KEY_BLACK[0], KEY_BLACK[1], KEY_BLACK[2], coverage(d, aa));
  }
}

function renderSize(size) {
  const rgba = Buffer.alloc(size * size * 4);
  const aa = 1 / size;
  const out = [0, 0, 0, 0];
  for (let y = 0; y < size; y += 1) {
    const fy = (y + 0.5) / size;
    for (let x = 0; x < size; x += 1) {
      const fx = (x + 0.5) / size;
      scene(fx, fy, aa, out);
      const offset = (y * size + x) * 4;
      rgba[offset] = Math.round(clamp(out[0], 0, 255));
      rgba[offset + 1] = Math.round(clamp(out[1], 0, 255));
      rgba[offset + 2] = Math.round(clamp(out[2], 0, 255));
      rgba[offset + 3] = Math.round(clamp(out[3], 0, 1) * 255);
    }
  }
  return rgba;
}

const ICNS_TYPES = {
  16: "icp4",
  32: "icp5",
  64: "icp6",
  128: "ic07",
  256: "ic08",
  512: "ic09",
  1024: "ic10",
};

const ICNS_RETINA = [
  { type: "ic11", size: 32 },
  { type: "ic12", size: 64 },
  { type: "ic13", size: 256 },
  { type: "ic14", size: 512 },
];

const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256];

function main() {
  const sizes = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
  const pngBySize = new Map();
  for (const size of sizes) {
    pngBySize.set(size, encodePng(renderSize(size), size));
  }

  const buildDir = join(root, "build");
  const resourcesDir = join(root, "resources");
  mkdirSync(buildDir, { recursive: true });
  mkdirSync(resourcesDir, { recursive: true });

  writeFileSync(join(buildDir, "icon.png"), pngBySize.get(1024));
  writeFileSync(join(resourcesDir, "icon.png"), pngBySize.get(256));

  const ico = buildIco(ICO_SIZES.map((size) => ({ size, png: pngBySize.get(size) })));
  writeFileSync(join(buildDir, "icon.ico"), ico);

  const icnsEntries = [];
  for (const [size, type] of Object.entries(ICNS_TYPES)) {
    icnsEntries.push({ type, png: pngBySize.get(Number(size)) });
  }
  for (const { type, size } of ICNS_RETINA) {
    icnsEntries.push({ type, png: pngBySize.get(size) });
  }
  writeFileSync(join(buildDir, "icon.icns"), buildIcns(icnsEntries));

  process.stdout.write(`Generated icons for sizes: ${sizes.join(", ")}\n`);
}

main();
