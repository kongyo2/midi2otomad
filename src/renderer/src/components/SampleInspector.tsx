import { useRef } from "react";
import { useStudio } from "../state/StudioContext";
import { Waveform } from "./Waveform";
import { midiToNoteName } from "../../../shared/music/pitch";
import { formatDb } from "../util/format";
import { previewSample } from "../audio/preview";
import {
  FILTER_TYPES,
  LFO_SHAPES,
  type Envelope,
  type Filter,
  type FilterType,
  type InterpolationMode,
  type LfoShape,
  type PitchMod,
  type Sample,
} from "../../../shared/schemas/project";

type NumericKeys<T> = Extract<{ [K in keyof T]: T[K] extends number ? K : never }[keyof T], string>;

interface NumField<T> {
  key: NumericKeys<T>;
  label: string;
  min: number;
  max: number;
  step: number;
  display: (value: number) => string;
}

const ms = (value: number): string => `${value.toFixed(0)} ms`;

const ENVELOPE_FIELDS: NumField<Envelope>[] = [
  { key: "delayMs", label: "ディレイ", min: 0, max: 2000, step: 1, display: ms },
  { key: "attackMs", label: "アタック", min: 0, max: 2000, step: 1, display: ms },
  { key: "holdMs", label: "ホールド", min: 0, max: 2000, step: 1, display: ms },
  { key: "decayMs", label: "ディケイ", min: 0, max: 4000, step: 1, display: ms },
  { key: "sustain", label: "サステイン", min: 0, max: 1, step: 0.01, display: (v) => `${Math.round(v * 100)}%` },
  { key: "releaseMs", label: "リリース", min: 0, max: 8000, step: 1, display: ms },
  { key: "attackCurve", label: "アタックカーブ", min: -8, max: 8, step: 0.1, display: (v) => v.toFixed(1) },
  { key: "decayCurve", label: "ディケイカーブ", min: -8, max: 8, step: 0.1, display: (v) => v.toFixed(1) },
  { key: "releaseCurve", label: "リリースカーブ", min: -8, max: 8, step: 0.1, display: (v) => v.toFixed(1) },
];

const oct = (value: number): string => `${value.toFixed(1)} oct`;

const FILTER_FIELDS: NumField<Filter>[] = [
  { key: "cutoffHz", label: "カットオフ", min: 20, max: 20000, step: 1, display: (v) => `${v.toFixed(0)} Hz` },
  { key: "q", label: "レゾナンス", min: 0.1, max: 24, step: 0.1, display: (v) => `Q ${v.toFixed(2)}` },
  { key: "gainDb", label: "フィルターゲイン", min: -24, max: 24, step: 0.5, display: (v) => `${v.toFixed(1)} dB` },
  { key: "envAmount", label: "フィルターEG", min: -8, max: 8, step: 0.1, display: oct },
  { key: "lfoDepth", label: "フィルターLFO深さ", min: 0, max: 8, step: 0.1, display: oct },
  { key: "lfoHz", label: "フィルターLFO速度", min: 0, max: 16, step: 0.1, display: (v) => `${v.toFixed(1)} Hz` },
];

const PITCH_FIELDS: NumField<PitchMod>[] = [
  { key: "glideSemitones", label: "グライド量", min: -36, max: 36, step: 1, display: (v) => `${v.toFixed(0)} st` },
  { key: "glideMs", label: "グライド時間", min: 0, max: 4000, step: 1, display: ms },
  { key: "glideCurve", label: "グライドカーブ", min: -8, max: 8, step: 0.1, display: (v) => v.toFixed(1) },
  { key: "vibratoCents", label: "ビブラート深さ", min: 0, max: 600, step: 1, display: (v) => `${v.toFixed(0)} cent` },
  { key: "vibratoHz", label: "ビブラート速度", min: 0, max: 16, step: 0.1, display: (v) => `${v.toFixed(1)} Hz` },
  { key: "vibratoDelayMs", label: "ビブラート遅延", min: 0, max: 2000, step: 1, display: ms },
  { key: "vibratoFadeMs", label: "ビブラートフェード", min: 0, max: 2000, step: 1, display: ms },
];

const FILTER_LABELS: Record<FilterType, string> = {
  lowpass: "ローパス",
  highpass: "ハイパス",
  bandpass: "バンドパス",
  notch: "ノッチ",
  peaking: "ピーキング",
  lowshelf: "ローシェルフ",
  highshelf: "ハイシェルフ",
  allpass: "オールパス",
};

const SHAPE_LABELS: Record<LfoShape, string> = {
  sine: "サイン",
  triangle: "三角",
  square: "矩形",
  saw: "ノコギリ",
};

function RangeField<T>({
  field,
  value,
  onChange,
}: {
  field: NumField<T>;
  value: number;
  onChange: (key: NumericKeys<T>, value: number) => void;
}): React.JSX.Element {
  return (
    <label className="field">
      <span className="field__label">
        {field.label} <em>{field.display(value)}</em>
      </span>
      <input
        className="range"
        type="range"
        aria-label={field.label}
        min={field.min}
        max={field.max}
        step={field.step}
        value={value}
        onChange={(event) => onChange(field.key, Number(event.target.value))}
      />
    </label>
  );
}

export function SampleInspector(): React.JSX.Element {
  const { project, selectedSampleId, updateSample, getPeaks, getAudio } = useStudio();
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

  const setEnvelope = (key: NumericKeys<Envelope>, value: number): void =>
    updateSample(sample.id, { envelope: { ...sample.envelope, [key]: value } });
  const setFilter = (key: NumericKeys<Filter>, value: number): void =>
    updateSample(sample.id, { filter: { ...sample.filter, [key]: value } });
  const setPitch = (key: NumericKeys<PitchMod>, value: number): void =>
    updateSample(sample.id, { pitchMod: { ...sample.pitchMod, [key]: value } });

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
          </span>
          <input
            className="range"
            type="range"
            aria-label="基準ピッチ"
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
            aria-label="微調整"
            min={-100}
            max={100}
            value={sample.tuneCents}
            onChange={(event) => updateSample(sample.id, { tuneCents: Number(event.target.value) })}
          />
        </label>
      </div>

      <div className="grid2">
        <label className="field">
          <span className="field__label">
            ゲイン <em>{formatDb(sample.gain)}</em>
          </span>
          <input
            className="range"
            type="range"
            aria-label="ゲイン"
            min={0}
            max={4}
            step={0.01}
            value={sample.gain}
            onChange={(event) => updateSample(sample.id, { gain: Number(event.target.value) })}
          />
        </label>
        <label className="field">
          <span className="field__label">補間方式</span>
          <select
            className="select"
            aria-label="補間方式"
            value={sample.interpolation}
            onChange={(event) => updateSample(sample.id, { interpolation: event.target.value as InterpolationMode })}
          >
            <option value="hermite">エルミート（高品質）</option>
            <option value="linear">リニア（軽量）</option>
          </select>
        </label>
      </div>

      <h3 className="subheading">エンベロープ (DAHDSR)</h3>
      <div className="grid2">
        {ENVELOPE_FIELDS.map((field) => (
          <RangeField key={field.key} field={field} value={sample.envelope[field.key]} onChange={setEnvelope} />
        ))}
      </div>

      <h3 className="subheading">音色フィルター</h3>
      <div className="grid2">
        <label className="checkline">
          <input
            type="checkbox"
            aria-label="フィルター"
            checked={sample.filter.enabled}
            onChange={(event) =>
              updateSample(sample.id, { filter: { ...sample.filter, enabled: event.target.checked } })
            }
          />
          有効
        </label>
        <label className="field">
          <span className="field__label">タイプ</span>
          <select
            className="select"
            aria-label="フィルタータイプ"
            value={sample.filter.type}
            onChange={(event) =>
              updateSample(sample.id, { filter: { ...sample.filter, type: event.target.value as FilterType } })
            }
          >
            {FILTER_TYPES.map((type) => (
              <option key={type} value={type}>
                {FILTER_LABELS[type]}
              </option>
            ))}
          </select>
        </label>
        {FILTER_FIELDS.map((field) => (
          <RangeField key={field.key} field={field} value={sample.filter[field.key]} onChange={setFilter} />
        ))}
        <label className="field">
          <span className="field__label">LFO波形</span>
          <select
            className="select"
            aria-label="フィルターLFO波形"
            value={sample.filter.lfoShape}
            onChange={(event) =>
              updateSample(sample.id, { filter: { ...sample.filter, lfoShape: event.target.value as LfoShape } })
            }
          >
            {LFO_SHAPES.map((shape) => (
              <option key={shape} value={shape}>
                {SHAPE_LABELS[shape]}
              </option>
            ))}
          </select>
        </label>
      </div>

      <h3 className="subheading">ダイナミックピッチ</h3>
      <div className="grid2">
        {PITCH_FIELDS.map((field) => (
          <RangeField key={field.key} field={field} value={sample.pitchMod[field.key]} onChange={setPitch} />
        ))}
        <label className="field">
          <span className="field__label">波形</span>
          <select
            className="select"
            aria-label="ビブラート波形"
            value={sample.pitchMod.vibratoShape}
            onChange={(event) =>
              updateSample(sample.id, {
                pitchMod: { ...sample.pitchMod, vibratoShape: event.target.value as LfoShape },
              })
            }
          >
            {LFO_SHAPES.map((shape) => (
              <option key={shape} value={shape}>
                {SHAPE_LABELS[shape]}
              </option>
            ))}
          </select>
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
