import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  type Project,
  type Sample,
  type Track,
  DEFAULT_BASE_PITCH,
  createEmptyProject,
} from "../../../shared/schemas/project";
import { type AudioBank, type MixResult, type PcmAudio, mixProject } from "../../../shared/audio/mixer";
import type { BounceResponse, ExportFormat, LoadedFile, WavBitDepth } from "../../../shared/media";
import { makeId } from "../../../shared/id";
import { buildWaveformPeaks, decodeAudio } from "../audio/decode";
import { PreviewEngine } from "../audio/engine";
import { midiToProject } from "../midi/import";

type Action =
  | { type: "setProject"; project: Project }
  | { type: "patchProject"; patch: Partial<Pick<Project, "name" | "bpm" | "masterGain">> }
  | { type: "addSample"; sample: Sample; assignToTracks: boolean }
  | { type: "updateSample"; id: string; patch: Partial<Sample> }
  | { type: "removeSample"; id: string }
  | { type: "updateTrack"; id: string; patch: Partial<Track> }
  | { type: "setNoteSample"; trackId: string; note: number; sampleId: string | null };

function projectReducer(project: Project, action: Action): Project {
  switch (action.type) {
    case "setProject":
      return action.project;
    case "patchProject":
      return { ...project, ...action.patch };
    case "addSample": {
      const samples = [...project.samples, action.sample];
      const tracks = action.assignToTracks
        ? project.tracks.map((track) =>
            track.defaultSampleId === null ? { ...track, defaultSampleId: action.sample.id } : track,
          )
        : project.tracks;
      return { ...project, samples, tracks };
    }
    case "updateSample":
      return {
        ...project,
        samples: project.samples.map((sample) => (sample.id === action.id ? { ...sample, ...action.patch } : sample)),
      };
    case "removeSample": {
      const samples = project.samples.filter((sample) => sample.id !== action.id);
      const tracks = project.tracks.map((track) => {
        const noteSampleMap = Object.fromEntries(
          Object.entries(track.noteSampleMap).filter(([, value]) => value !== action.id),
        );
        return {
          ...track,
          defaultSampleId: track.defaultSampleId === action.id ? null : track.defaultSampleId,
          noteSampleMap,
        };
      });
      return { ...project, samples, tracks };
    }
    case "updateTrack":
      return {
        ...project,
        tracks: project.tracks.map((track) => (track.id === action.id ? { ...track, ...action.patch } : track)),
      };
    case "setNoteSample":
      return {
        ...project,
        tracks: project.tracks.map((track) => {
          if (track.id !== action.trackId) {
            return track;
          }
          const noteSampleMap = { ...track.noteSampleMap };
          if (action.sampleId === null) {
            delete noteSampleMap[String(action.note)];
          } else {
            noteSampleMap[String(action.note)] = action.sampleId;
          }
          return { ...track, noteSampleMap };
        }),
      };
    default:
      return project;
  }
}

export interface ExportOptions {
  format: ExportFormat;
  wavBitDepth?: WavBitDepth;
  mp3Bitrate?: number;
}

export interface StudioContextValue {
  project: Project;
  selectedTrackId: string | null;
  selectedSampleId: string | null;
  isPlaying: boolean;
  busy: string | null;
  toast: string | null;
  engineRef: React.RefObject<PreviewEngine | null>;
  selectTrack: (id: string | null) => void;
  selectSample: (id: string | null) => void;
  importMidiBytes: (bytes: Uint8Array, name: string) => void;
  ingestAudio: (files: Array<LoadedFile | File>, assignTrackId?: string) => Promise<void>;
  updateSample: (id: string, patch: Partial<Sample>) => void;
  removeSample: (id: string) => void;
  patchProject: (patch: Partial<Pick<Project, "name" | "bpm" | "masterGain">>) => void;
  updateTrack: (id: string, patch: Partial<Track>) => void;
  setNoteSample: (trackId: string, note: number, sampleId: string | null) => void;
  getAudio: (sampleId: string) => PcmAudio | undefined;
  getPeaks: (sampleId: string) => Float32Array | undefined;
  play: (fromSec?: number) => void;
  pause: () => void;
  stop: () => void;
  togglePlay: () => void;
  seek: (sec: number) => void;
  exportMix: (options: ExportOptions) => Promise<void>;
  showToast: (message: string) => void;
}

const StudioContext = createContext<StudioContextValue | null>(null);

async function toNameBytes(file: LoadedFile | File): Promise<{ name: string; bytes: Uint8Array }> {
  if (file instanceof File) {
    const buffer = await file.arrayBuffer();
    return { name: file.name, bytes: new Uint8Array(buffer) };
  }
  return { name: file.name, bytes: file.data };
}

export function StudioProvider({ children }: { children: ReactNode }): React.JSX.Element {
  const [project, dispatch] = useReducer(projectReducer, undefined, () => createEmptyProject());
  const [selectedTrackId, setSelectedTrackId] = useState<string | null>(null);
  const [selectedSampleId, setSelectedSampleId] = useState<string | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const bankRef = useRef<Map<string, PcmAudio>>(new Map());
  const peaksRef = useRef<Map<string, Float32Array>>(new Map());
  const engineRef = useRef<PreviewEngine | null>(null);
  const dirtyRef = useRef(true);
  const projectRef = useRef(project);
  projectRef.current = project;
  const toastTimer = useRef<number | null>(null);

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) {
      window.clearTimeout(toastTimer.current);
    }
    toastTimer.current = window.setTimeout(() => setToast(null), 3200);
  }, []);

  const markDirty = useCallback(() => {
    dirtyRef.current = true;
  }, []);

  const getEngine = useCallback((): PreviewEngine => {
    if (engineRef.current === null) {
      const engine = new PreviewEngine();
      engine.onEnded = () => setIsPlaying(false);
      engine.setMasterGain(projectRef.current.masterGain);
      engineRef.current = engine;
    }
    return engineRef.current;
  }, []);

  const bank = useMemo<AudioBank>(() => ({ get: (id) => bankRef.current.get(id) }), []);

  const ensureMix = useCallback((): MixResult => {
    const engine = getEngine();
    const mix = mixProject(projectRef.current, bank, { limiter: true });
    if (dirtyRef.current) {
      engine.setMix(mix);
      dirtyRef.current = false;
    }
    return mix;
  }, [bank, getEngine]);

  useEffect(() => {
    engineRef.current?.setMasterGain(project.masterGain);
  }, [project.masterGain]);

  const selectTrack = useCallback((id: string | null) => setSelectedTrackId(id), []);
  const selectSample = useCallback((id: string | null) => setSelectedSampleId(id), []);

  const importMidiBytes = useCallback(
    (bytes: Uint8Array, name: string) => {
      try {
        const { project: next, trackCount, noteCount } = midiToProject(bytes, name, projectRef.current);
        dispatch({ type: "setProject", project: next });
        markDirty();
        setSelectedTrackId(next.tracks[0]?.id ?? null);
        showToast(`${name} を読み込みました — ${trackCount} トラック / ${noteCount} ノート`);
      } catch (error) {
        showToast(`MIDI 読み込みに失敗しました: ${error instanceof Error ? error.message : String(error)}`);
      }
    },
    [markDirty, showToast],
  );

  const ingestAudio = useCallback(
    async (files: Array<LoadedFile | File>, assignTrackId?: string) => {
      setBusy("音声素材をデコード中…");
      try {
        const decoded = await Promise.all(
          files.map(async (file) => {
            const { name, bytes } = await toNameBytes(file);
            return { name, pcm: await decodeAudio(bytes) };
          }),
        );
        let firstId: string | null = null;
        for (const { name, pcm } of decoded) {
          const id = makeId("sample");
          bankRef.current.set(id, pcm);
          peaksRef.current.set(id, buildWaveformPeaks(pcm));
          const durationSec = pcm.frames / pcm.sampleRate;
          const isFirstSampleEver = projectRef.current.samples.length === 0 && firstId === null;
          const sample: Sample = {
            id,
            name: name.replace(/\.[^.]+$/, ""),
            fileName: name,
            basePitch: DEFAULT_BASE_PITCH,
            tuneCents: 0,
            gain: 1,
            durationSec,
            loop: { enabled: false, startSec: 0, endSec: durationSec },
            envelope: { attackMs: 4, releaseMs: 90 },
          };
          dispatch({ type: "addSample", sample, assignToTracks: isFirstSampleEver });
          markDirty();
          if (firstId === null) {
            firstId = id;
          }
        }
        if (assignTrackId !== undefined && firstId !== null) {
          dispatch({ type: "updateTrack", id: assignTrackId, patch: { defaultSampleId: firstId } });
          markDirty();
        }
        if (firstId !== null) {
          setSelectedSampleId(firstId);
          showToast(`${files.length} 個の音声素材を追加しました`);
        }
      } catch (error) {
        showToast(`音声の読み込みに失敗しました: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setBusy(null);
      }
    },
    [markDirty, showToast],
  );

  const updateSample = useCallback(
    (id: string, patch: Partial<Sample>) => {
      dispatch({ type: "updateSample", id, patch });
      markDirty();
    },
    [markDirty],
  );

  const removeSample = useCallback(
    (id: string) => {
      bankRef.current.delete(id);
      peaksRef.current.delete(id);
      dispatch({ type: "removeSample", id });
      markDirty();
      setSelectedSampleId((current) => (current === id ? null : current));
    },
    [markDirty],
  );

  const patchProject = useCallback(
    (patch: Partial<Pick<Project, "name" | "bpm" | "masterGain">>) => {
      dispatch({ type: "patchProject", patch });
      if (patch.bpm !== undefined) {
        markDirty();
      }
    },
    [markDirty],
  );

  const updateTrack = useCallback(
    (id: string, patch: Partial<Track>) => {
      dispatch({ type: "updateTrack", id, patch });
      markDirty();
    },
    [markDirty],
  );

  const setNoteSample = useCallback(
    (trackId: string, note: number, sampleId: string | null) => {
      dispatch({ type: "setNoteSample", trackId, note, sampleId });
      markDirty();
    },
    [markDirty],
  );

  const getAudio = useCallback((sampleId: string) => bankRef.current.get(sampleId), []);
  const getPeaks = useCallback((sampleId: string) => peaksRef.current.get(sampleId), []);

  const play = useCallback(
    (fromSec?: number) => {
      ensureMix();
      const engine = getEngine();
      engine.play(fromSec);
      setIsPlaying(true);
    },
    [ensureMix, getEngine],
  );

  const pause = useCallback(() => {
    engineRef.current?.pause();
    setIsPlaying(false);
  }, []);

  const stop = useCallback(() => {
    engineRef.current?.stop();
    setIsPlaying(false);
  }, []);

  const togglePlay = useCallback(() => {
    if (engineRef.current?.transport === "playing") {
      pause();
    } else {
      play();
    }
  }, [pause, play]);

  const seek = useCallback(
    (sec: number) => {
      ensureMix();
      getEngine().seek(sec);
    },
    [ensureMix, getEngine],
  );

  const exportMix = useCallback(
    async (options: ExportOptions) => {
      setBusy("ミックスを書き出し中…");
      try {
        const mix = mixProject(projectRef.current, bank, { limiter: true });
        if (mix.frames < 2) {
          showToast("書き出す音がありません。MIDI と音声素材を読み込んでください。");
          return;
        }
        const request = {
          format: options.format,
          defaultName: projectRef.current.name,
          pcm: { sampleRate: mix.sampleRate, left: mix.left, right: mix.right, frames: mix.frames },
          ...(options.wavBitDepth !== undefined ? { wavBitDepth: options.wavBitDepth } : {}),
          ...(options.mp3Bitrate !== undefined ? { mp3Bitrate: options.mp3Bitrate } : {}),
        };
        const response: BounceResponse = await window.api.bounce(request);
        if (response.ok) {
          const kb = Math.round(response.bytes / 1024);
          showToast(`書き出し完了: ${response.path} (${kb.toLocaleString()} KB)`);
        } else if (response.canceled) {
          showToast("書き出しをキャンセルしました");
        } else {
          showToast(`書き出しに失敗しました: ${response.error ?? "unknown"}`);
        }
      } catch (error) {
        showToast(`書き出しに失敗しました: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setBusy(null);
      }
    },
    [bank, showToast],
  );

  const value = useMemo<StudioContextValue>(
    () => ({
      project,
      selectedTrackId,
      selectedSampleId,
      isPlaying,
      busy,
      toast,
      engineRef,
      selectTrack,
      selectSample,
      importMidiBytes,
      ingestAudio,
      updateSample,
      removeSample,
      patchProject,
      updateTrack,
      setNoteSample,
      getAudio,
      getPeaks,
      play,
      pause,
      stop,
      togglePlay,
      seek,
      exportMix,
      showToast,
    }),
    [
      project,
      selectedTrackId,
      selectedSampleId,
      isPlaying,
      busy,
      toast,
      selectTrack,
      selectSample,
      importMidiBytes,
      ingestAudio,
      updateSample,
      removeSample,
      patchProject,
      updateTrack,
      setNoteSample,
      getAudio,
      getPeaks,
      play,
      pause,
      stop,
      togglePlay,
      seek,
      exportMix,
      showToast,
    ],
  );

  return <StudioContext.Provider value={value}>{children}</StudioContext.Provider>;
}

export function useStudio(): StudioContextValue {
  const value = useContext(StudioContext);
  if (value === null) {
    throw new Error("useStudio must be used within a StudioProvider");
  }
  return value;
}

export function usePlayhead(): number {
  const { engineRef } = useStudio();
  const [position, setPosition] = useState(0);
  useEffect(() => {
    let raf = 0;
    const tick = (): void => {
      const engine = engineRef.current;
      if (engine !== null) {
        setPosition(engine.getPosition());
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [engineRef]);
  return position;
}
