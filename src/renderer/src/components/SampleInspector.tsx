import { useRef } from "react";
import { useStudio } from "../state/StudioContext";
import { Waveform } from "./Waveform";
import { midiToNoteName } from "../../../shared/music/pitch";
import { detectSamplePitch } from "../../../shared/music/detect";
import { formatDb } from "../util/format";
import { previewSample } from "../audio/preview";
import type { Sample } from "../../../shared/schemas/project";

export function SampleInspector(): React.JSX.Element {
  const { project, selectedSampleId, updateSample, getPeaks, getAudio, showToast } = useStudio();
  const sample = project.samples.find((item) => item.id === selectedSampleId);

  if (sample === undefined) {
    return (
      <section className="panel">
        <h2 className="panel__heading">素材エディタ</h2>
        <p className="panel__muted">
          ライブラリから音声素材を選択すると、基準ピッチ・エンベロープ・ループを編集できます。
        </p>
      </section>
    );
  }

  const peaks = getPeaks(sample.id);
  const duration = sample.durationSec > 0 ? sample.durationSec : 1;
  const loopEnd = sample.loop.endSec > sample.loop.startSec ? sample.loop.endSec : duration;

  const onPreview = (): void => {
    const pcm = getAudio(sample.id);
    if (pcm !== undefined) {
      previewSample(pcm, sample);
    }
  };

  const onDetectPitch = (): void => {
    const pcm = getAudio(sample.id);
    if (pcm === undefined) {
      showToast("音声がまだデコードされていません");
      return;
    }
    const estimate = detectSamplePitch(pcm);
    if (estimate === null) {
      showToast("ピッチを検出できませんでした");
      return;
    }
    updateSample(sample.id, { basePitch: estimate.basePitch, tuneCents: estimate.tuneCents });
    const sign = estimate.tuneCents >= 0 ? "+" : "";
    const confidence = Math.round(estimate.probability * 100);
    showToast(
      `ピッチを検出: ${midiToNoteName(estimate.basePitch)} ${sign}${estimate.tuneCents} cent（確度 ${confidence}%）`,
    );
  };

  return (
    <section className="panel">
      <div className="panel__head">
        <h2 className="panel__heading">素材エディタ</h2>
        <button type="button" className="btn btn--sm" onClick={onPreview}>
          ▶ 試聴
        </button>
      </div>

      <label className="field">
        <span className="field__label">名前</span>
        <input
          className="input"
          value={sample.name}
          onChange={(event) => updateSample(sample.id, { name: event.target.value })}
        />
      </label>

      <LoopEditor sample={sample} peaks={peaks} duration={duration} loopEnd={loopEnd} />

      <div className="grid2">
        <label className="field">
          <span className="field__label">
            基準ピッチ <em>{midiToNoteName(sample.basePitch)}</em>
            <button type="button" className="linkbtn" onClick={onDetectPitch}>
              🎯 自動検出
            </button>
          </span>
          <input
            className="range"
            type="range"
            min={24}
            max={96}
            value={sample.basePitch}
            onChange={(event) => updateSample(sample.id, { basePitch: Number(event.target.value) })}
          />
        </label>
        <label className="field">
          <span className="field__label">
            微調整 <em>{sample.tuneCents.toFixed(0)} cent</em>
          </span>
          <input
            className="range"
            type="range"
            min={-100}
            max={100}
            value={sample.tuneCents}
            onChange={(event) => updateSample(sample.id, { tuneCents: Number(event.target.value) })}
          />
        </label>
      </div>

      <label className="field">
        <span className="field__label">
          ゲイン <em>{formatDb(sample.gain)}</em>
        </span>
        <input
          className="range"
          type="range"
          min={0}
          max={4}
          step={0.01}
          value={sample.gain}
          onChange={(event) => updateSample(sample.id, { gain: Number(event.target.value) })}
        />
      </label>

      <h3 className="subheading">エンベロープ</h3>
      <div className="grid2">
        <label className="field">
          <span className="field__label">
            アタック <em>{sample.envelope.attackMs.toFixed(0)} ms</em>
          </span>
          <input
            className="range"
            type="range"
            min={0}
            max={1000}
            value={sample.envelope.attackMs}
            onChange={(event) =>
              updateSample(sample.id, { envelope: { ...sample.envelope, attackMs: Number(event.target.value) } })
            }
          />
        </label>
        <label className="field">
          <span className="field__label">
            リリース <em>{sample.envelope.releaseMs.toFixed(0)} ms</em>
          </span>
          <input
            className="range"
            type="range"
            min={0}
            max={4000}
            value={sample.envelope.releaseMs}
            onChange={(event) =>
              updateSample(sample.id, { envelope: { ...sample.envelope, releaseMs: Number(event.target.value) } })
            }
          />
        </label>
      </div>
    </section>
  );
}

interface LoopEditorProps {
  sample: Sample;
  peaks: Float32Array | undefined;
  duration: number;
  loopEnd: number;
}

function LoopEditor({ sample, peaks, duration, loopEnd }: LoopEditorProps): React.JSX.Element {
  const { updateSample } = useStudio();
  const trackRef = useRef<HTMLDivElement | null>(null);
  const startFrac = Math.min(1, Math.max(0, sample.loop.startSec / duration));
  const endFrac = Math.min(1, Math.max(0, loopEnd / duration));

  const beginDrag = (handle: "start" | "end") => (event: React.PointerEvent) => {
    event.preventDefault();
    const element = trackRef.current!;
    const move = (clientX: number): void => {
      const rect = element.getBoundingClientRect();
      const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
      const sec = frac * duration;
      if (handle === "start") {
        updateSample(sample.id, {
          loop: { ...sample.loop, startSec: Math.min(sec, loopEnd - 0.001) },
        });
      } else {
        updateSample(sample.id, {
          loop: { ...sample.loop, endSec: Math.max(sec, sample.loop.startSec + 0.001) },
        });
      }
    };
    const onMove = (ev: PointerEvent): void => move(ev.clientX);
    const onUp = (): void => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  return (
    <div className="loopeditor">
      <div className="loopeditor__head">
        <label className="checkline">
          <input
            type="checkbox"
            checked={sample.loop.enabled}
            onChange={(event) => updateSample(sample.id, { loop: { ...sample.loop, enabled: event.target.checked } })}
          />
          ループ（ロングトーン対応）
        </label>
        <button
          type="button"
          className="linkbtn"
          onClick={() => updateSample(sample.id, { loop: { ...sample.loop, startSec: 0, endSec: duration } })}
        >
          全体
        </button>
      </div>
      <div className="loopeditor__wave" ref={trackRef}>
        <Waveform peaks={peaks} loop={{ startFrac, endFrac, enabled: sample.loop.enabled }} color={"#9d86ff"} />
        {sample.loop.enabled ? (
          <>
            <span className="loophandle" style={{ left: `${startFrac * 100}%` }} onPointerDown={beginDrag("start")} />
            <span className="loophandle" style={{ left: `${endFrac * 100}%` }} onPointerDown={beginDrag("end")} />
          </>
        ) : null}
      </div>
      <div className="loopeditor__times">
        <span>start {sample.loop.startSec.toFixed(3)}s</span>
        <span>end {loopEnd.toFixed(3)}s</span>
      </div>
    </div>
  );
}
