import { useMemo, useState } from "react";
import { useStudio, usePlayhead } from "../state/StudioContext";
import { LevelMeter } from "./LevelMeter";
import { formatTime } from "../util/format";
import type { ExportFormat, WavBitDepth } from "../../../shared/media";

export function TopBar(): React.JSX.Element {
  const { project, isPlaying, togglePlay, stop, seek, patchProject, exportMix, busy, importMidiBytes } = useStudio();
  const playhead = usePlayhead();
  const [format, setFormat] = useState<ExportFormat>("wav");
  const [wavBitDepth, setWavBitDepth] = useState<WavBitDepth>(24);
  const [mp3Bitrate, setMp3Bitrate] = useState(320);

  const duration = useMemo(() => {
    let end = 0;
    for (const track of project.tracks) {
      for (const note of track.notes) {
        end = Math.max(end, note.startSec + note.durationSec);
      }
    }
    return end;
  }, [project.tracks]);

  const onOpenMidi = async (): Promise<void> => {
    const file = await window.api.openMidi();
    if (file !== null) {
      importMidiBytes(file.data, file.name);
    }
  };

  const onExport = (): void => {
    void exportMix({
      format,
      ...(format === "wav" ? { wavBitDepth } : {}),
      ...(format === "mp3" ? { mp3Bitrate } : {}),
    });
  };

  return (
    <header className="topbar">
      <div className="topbar__brand">
        <span className="topbar__logo">🎹</span>
        <div>
          <h1 className="topbar__title">midi2otomad</h1>
          <p className="topbar__tag">MIDI 音MAD スタジオ</p>
        </div>
      </div>

      <div className="topbar__group">
        <button type="button" className="btn btn--ghost" onClick={() => void onOpenMidi()}>
          MIDI を開く
        </button>
      </div>

      <div className="topbar__transport">
        <button type="button" className="transportbtn" title="先頭へ" onClick={() => seek(0)}>
          ⏮
        </button>
        <button type="button" className="transportbtn transportbtn--main" onClick={togglePlay}>
          {isPlaying ? "⏸" : "▶"}
        </button>
        <button type="button" className="transportbtn" title="停止" onClick={stop}>
          ⏹
        </button>
        <span className="topbar__time">
          {formatTime(playhead)} <span className="topbar__time-sep">/</span> {formatTime(duration)}
        </span>
        <LevelMeter />
      </div>

      <div className="topbar__group topbar__master">
        <label className="microfield">
          <span>BPM</span>
          <input
            className="input input--mini"
            type="number"
            min={20}
            max={400}
            value={Math.round(project.bpm)}
            onChange={(event) => patchProject({ bpm: Number(event.target.value) })}
          />
        </label>
        <label className="microfield microfield--wide">
          <span>Master</span>
          <input
            className="range"
            type="range"
            min={0}
            max={2}
            step={0.01}
            value={project.masterGain}
            onChange={(event) => patchProject({ masterGain: Number(event.target.value) })}
          />
        </label>
      </div>

      <div className="topbar__export">
        <select
          className="select select--mini"
          value={format}
          onChange={(event) => setFormat(event.target.value as ExportFormat)}
        >
          <option value="wav">WAV</option>
          <option value="mp3">MP3</option>
        </select>
        {format === "wav" ? (
          <select
            className="select select--mini"
            value={wavBitDepth}
            onChange={(event) => setWavBitDepth(Number(event.target.value) as WavBitDepth)}
          >
            <option value={16}>16-bit</option>
            <option value={24}>24-bit</option>
            <option value={32}>32-bit float</option>
          </select>
        ) : (
          <select
            className="select select--mini"
            value={mp3Bitrate}
            onChange={(event) => setMp3Bitrate(Number(event.target.value))}
          >
            <option value={192}>192k</option>
            <option value={256}>256k</option>
            <option value={320}>320k</option>
          </select>
        )}
        <button type="button" className="btn" disabled={busy !== null} onClick={onExport}>
          {busy === null ? "⬇ 書き出し" : "処理中…"}
        </button>
      </div>
    </header>
  );
}
