import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const outDir = process.argv[2] || join(tmpdir(), "m2o-fixtures");
mkdirSync(outDir, { recursive: true });

function vlq(n) {
  const bytes = [n & 0x7f];
  n >>= 7;
  while (n > 0) {
    bytes.unshift((n & 0x7f) | 0x80);
    n >>= 7;
  }
  return bytes;
}

function smf() {
  const ppq = 480;
  const ev = [];
  ev.push(0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20);
  for (const key of [60, 64, 67]) {
    ev.push(0x00, 0x90, key, 100);
    ev.push(...vlq(ppq), 0x80, key, 0);
  }
  ev.push(0x00, 0xff, 0x2f, 0x00);
  const head = [
    0x4d, 0x54, 0x68, 0x64, 0, 0, 0, 6, 0, 0, 0, 1,
    (ppq >> 8) & 0xff, ppq & 0xff,
  ];
  const len = ev.length;
  const trk = [
    0x4d, 0x54, 0x72, 0x6b,
    (len >> 24) & 0xff, (len >> 16) & 0xff, (len >> 8) & 0xff, len & 0xff,
    ...ev,
  ];
  return Buffer.from([...head, ...trk]);
}

function wav() {
  const rate = 48000, secs = 0.4, freq = 440;
  const n = Math.floor(rate * secs);
  const data = Buffer.alloc(n * 4);
  for (let i = 0; i < n; i++) {
    const env = Math.min(1, i / 200, (n - i) / 200);
    const s = Math.round(Math.sin((2 * Math.PI * freq * i) / rate) * env * 0.7 * 32767);
    data.writeInt16LE(s, i * 4);
    data.writeInt16LE(s, i * 4 + 2);
  }
  const h = Buffer.alloc(44);
  h.write("RIFF", 0);
  h.writeUInt32LE(36 + data.length, 4);
  h.write("WAVE", 8);
  h.write("fmt ", 12);
  h.writeUInt32LE(16, 16);
  h.writeUInt16LE(1, 20);
  h.writeUInt16LE(2, 22);
  h.writeUInt32LE(rate, 24);
  h.writeUInt32LE(rate * 4, 28);
  h.writeUInt16LE(4, 32);
  h.writeUInt16LE(16, 34);
  h.write("data", 36);
  h.writeUInt32LE(data.length, 40);
  return Buffer.concat([h, data]);
}

const mid = join(outDir, "melody.mid");
const wv = join(outDir, "tone.wav");
writeFileSync(mid, smf());
writeFileSync(wv, wav());
console.log(mid);
console.log(wv);
