import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseProject } from "../src/shared/schemas/project";
import { bankFromRecord, mixProject, type PcmAudio } from "../src/shared/audio/mixer";
import { encodeWav, writeExport } from "../src/main/media/encode";
import { pitchRatio } from "../src/shared/music/pitch";
import { midiToProject } from "../src/renderer/src/midi/import";
import { Midi } from "@tonejs/midi";

let failures = 0;
function check(name: string, condition: boolean, detail = ""): void {
  if (condition) {
    console.log(`  ok   ${name}`);
  } else {
    failures += 1;
    console.error(`  FAIL ${name} ${detail}`);
  }
}

function makeSine(freq: number, seconds: number, sampleRate: number): PcmAudio {
  const frames = Math.round(seconds * sampleRate);
  const data = new Float32Array(frames);
  for (let i = 0; i < frames; i += 1) {
    data[i] = Math.sin((2 * Math.PI * freq * i) / sampleRate) * 0.8;
  }
  return { sampleRate, channels: [data], frames };
}

function allFinite(arr: Float32Array): boolean {
  for (let i = 0; i < arr.length; i += 1) {
    if (!Number.isFinite(arr[i] ?? 0)) {
      return false;
    }
  }
  return true;
}

async function main(): Promise<void> {
  console.log("pitch math");
  check("octave up ratio is 2", Math.abs(pitchRatio(72, 60) - 2) < 1e-9);
  check("unison ratio is 1", Math.abs(pitchRatio(60, 60) - 1) < 1e-9);
  check("tune cents", Math.abs(pitchRatio(60, 60, 1200) - 2) < 1e-9);

  const sampleRate = 48000;
  const sine = makeSine(220, 1, sampleRate);

  const project = parseProject({
    version: 1,
    name: "selftest",
    bpm: 120,
    ppq: 480,
    sampleRate,
    masterGain: 1,
    tempos: [],
    samples: [
      {
        id: "s1",
        name: "sine",
        basePitch: 60,
        gain: 1,
        durationSec: 1,
        loop: { enabled: true, startSec: 0.1, endSec: 0.9 },
        envelope: { attackMs: 5, releaseMs: 120 },
      },
    ],
    tracks: [
      {
        id: "t1",
        name: "lead",
        defaultSampleId: "s1",
        notes: [
          { pitch: 60, startSec: 0, durationSec: 0.5, velocity: 100 },
          { pitch: 67, startSec: 0.5, durationSec: 0.5, velocity: 80 },
          { pitch: 72, startSec: 1.0, durationSec: 1.5, velocity: 120 },
        ],
        dynamics: {
          volume: [],
          expression: [
            { t: 0, v: 1 },
            { t: 2.5, v: 0.3 },
          ],
        },
      },
    ],
  });

  console.log("mixer");
  const bank = bankFromRecord({ s1: sine });
  const mix = mixProject(project, bank);
  const expectedEnd = 1.0 + 1.5 + 0.12 + 0.25;
  check(
    "frame count covers the long note + release + tail",
    Math.abs(mix.frames / sampleRate - expectedEnd) < 0.05,
    `got ${mix.frames / sampleRate}s`,
  );
  check("peak is audible and bounded", mix.peak > 0.05 && mix.peak <= 1.0001, `peak=${mix.peak}`);
  check("left channel has no NaN/Inf", allFinite(mix.left));
  check("right channel has no NaN/Inf", allFinite(mix.right));
  const firstNoteRms = (() => {
    let sum = 0;
    const n = Math.round(0.4 * sampleRate);
    for (let i = 0; i < n; i += 1) {
      sum += (mix.left[i] ?? 0) ** 2;
    }
    return Math.sqrt(sum / n);
  })();
  check("first note produces signal", firstNoteRms > 0.01, `rms=${firstNoteRms}`);

  console.log("midi import");
  const authored = new Midi();
  const authoredTrack = authored.addTrack();
  authoredTrack.name = "Melody";
  authoredTrack.addNote({ midi: 60, time: 0, duration: 0.5, velocity: 0.8 });
  authoredTrack.addNote({ midi: 67, time: 0.5, duration: 0.5, velocity: 0.6 });
  authoredTrack.addCC({ number: 11, value: 0.9, time: 0 });
  authoredTrack.addCC({ number: 11, value: 0.2, time: 0.8 });
  const midiBytes = authored.toArray();
  const imported = midiToProject(midiBytes, "demo.mid");
  check("imported one track", imported.trackCount === 1, `tracks=${imported.trackCount}`);
  check("imported two notes", imported.noteCount === 2, `notes=${imported.noteCount}`);
  const importedTrack = imported.project.tracks[0];
  check("first note pitch is C4 (60)", importedTrack?.notes[0]?.pitch === 60);
  check("second note starts ~0.5s", Math.abs((importedTrack?.notes[1]?.startSec ?? 0) - 0.5) < 0.02);
  check("expression CC11 captured", (importedTrack?.dynamics.expression.length ?? 0) >= 2);
  check("project name from filename", imported.project.name === "demo");

  console.log("mute / solo");
  const muted = parseProject({ ...project, tracks: [{ ...project.tracks[0], muted: true }] });
  const mutedMix = mixProject(muted, bank);
  check("muted track is silent", mutedMix.peak < 1e-6);

  console.log("automation defaults");
  const rmsOf = (m: { left: Float32Array }, sec: number): number => {
    let sum = 0;
    const n = Math.round(sec * sampleRate);
    for (let i = 0; i < n; i += 1) {
      sum += (m.left[i] ?? 0) ** 2;
    }
    return Math.sqrt(sum / n);
  };
  const baseSample = {
    id: "s1",
    name: "sine",
    basePitch: 60,
    gain: 1,
    durationSec: 1,
    loop: { enabled: false, startSec: 0, endSec: 0 },
    envelope: { attackMs: 1, releaseMs: 5 },
  };
  const introNote = { pitch: 60, startSec: 0, durationSec: 0.3, velocity: 100 };
  const lateCc = parseProject({
    version: 1,
    name: "latecc",
    sampleRate,
    samples: [baseSample],
    tracks: [
      {
        id: "t1",
        name: "x",
        defaultSampleId: "s1",
        notes: [introNote],
        dynamics: { volume: [], expression: [{ t: 1, v: 0.1 }] },
      },
    ],
  });
  const noCc = parseProject({
    version: 1,
    name: "nocc",
    sampleRate,
    samples: [baseSample],
    tracks: [
      { id: "t1", name: "x", defaultSampleId: "s1", notes: [introNote], dynamics: { volume: [], expression: [] } },
    ],
  });
  const lateRms = rmsOf(mixProject(lateCc, bank), 0.2);
  const noRms = rmsOf(mixProject(noCc, bank), 0.2);
  check(
    "intro before a late CC stays at full volume",
    Math.abs(lateRms - noRms) < noRms * 0.05,
    `late=${lateRms} plain=${noRms}`,
  );

  console.log("silence detection");
  const noSample = parseProject({
    version: 1,
    name: "silent",
    sampleRate,
    samples: [],
    tracks: [
      { id: "t1", name: "x", defaultSampleId: null, notes: [introNote], dynamics: { volume: [], expression: [] } },
    ],
  });
  const silentMix = mixProject(noSample, bank);
  check("project with no assigned sample has zero peak", silentMix.peak < 1e-6, `peak=${silentMix.peak}`);
  check("frame count alone would miss the silence", silentMix.frames > 2);

  console.log("wav encode");
  const wav24 = encodeWav({ sampleRate, left: mix.left, right: mix.right, frames: mix.frames }, 24);
  check("RIFF tag", wav24.toString("ascii", 0, 4) === "RIFF");
  check("WAVE tag", wav24.toString("ascii", 8, 12) === "WAVE");
  check("24-bit size matches", wav24.byteLength === 44 + mix.frames * 6, `len=${wav24.byteLength}`);
  const wav32 = encodeWav({ sampleRate, left: mix.left, right: mix.right, frames: mix.frames }, 32);
  check("float wav has fact chunk", wav32.includes(Buffer.from("fact", "ascii")));

  const dir = await mkdtemp(join(tmpdir(), "otomad-selftest-"));
  try {
    console.log("file export: wav");
    const wavOut = join(dir, "out.wav");
    const wavResult = await writeExport(
      { sampleRate, left: mix.left, right: mix.right, frames: mix.frames },
      { format: "wav", path: wavOut, wavBitDepth: 24 },
    );
    check("wav file written", wavResult.bytes > 1000, `bytes=${wavResult.bytes}`);

    console.log("file export: mp3 (node-av / libmp3lame)");
    const mp3Out = join(dir, "out.mp3");
    const mp3Result = await writeExport(
      { sampleRate, left: mix.left, right: mix.right, frames: mix.frames },
      { format: "mp3", path: mp3Out, mp3Bitrate: 256 },
    );
    check("mp3 file is non-trivial", mp3Result.bytes > 2000, `bytes=${mp3Result.bytes}`);
    const head = await readFile(mp3Out);
    const isId3 = head.toString("ascii", 0, 3) === "ID3";
    const isFrameSync = head.length > 1 && head[0] === 0xff && (head[1] ?? 0) >= 0xe0;
    check("mp3 has valid header (ID3 or frame sync)", isId3 || isFrameSync, `b0=${head[0]} b1=${head[1]}`);

    console.log("mp3 round-trip probe");
    const { Demuxer } = await import("node-av");
    const input = await Demuxer.open(mp3Out);
    const audio = input.audio();
    check("decoded stream is audio", audio !== null && audio !== undefined);
    await input.close();
  } finally {
    await rm(dir, { recursive: true, force: true });
  }

  console.log("");
  if (failures > 0) {
    console.error(`${failures} check(s) failed`);
    process.exit(1);
  }
  console.log("all checks passed");
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
