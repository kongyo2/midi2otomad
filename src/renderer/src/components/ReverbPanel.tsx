import { useStudio } from "../state/StudioContext";
import type { Reverb } from "../../../shared/schemas/project";

type NumericKeys<T> = Extract<{ [K in keyof T]: T[K] extends number ? K : never }[keyof T], string>;

interface ReverbField {
  key: NumericKeys<Reverb>;
  label: string;
  min: number;
  max: number;
  step: number;
  display: (value: number) => string;
}

const pct = (value: number): string => `${Math.round(value * 100)}%`;

const REVERB_FIELDS: ReverbField[] = [
  { key: "roomSize", label: "ルームサイズ", min: 0, max: 1, step: 0.01, display: pct },
  { key: "damping", label: "ダンピング", min: 0, max: 1, step: 0.01, display: pct },
  { key: "width", label: "ステレオ幅", min: 0, max: 1, step: 0.01, display: pct },
  { key: "wet", label: "ウェット量", min: 0, max: 1, step: 0.01, display: pct },
  { key: "preDelayMs", label: "プリディレイ", min: 0, max: 500, step: 1, display: (v) => `${v.toFixed(0)} ms` },
];

export function ReverbPanel(): React.JSX.Element {
  const { project, patchProject } = useStudio();
  const reverb = project.reverb;
  const set = (patch: Partial<Reverb>): void => patchProject({ reverb: { ...reverb, ...patch } });

  return (
    <section className="panel">
      <div className="panel__head">
        <h2 className="panel__heading">マスターリバーブ</h2>
        <label className="checkline">
          <input
            type="checkbox"
            aria-label="リバーブ"
            checked={reverb.enabled}
            onChange={(event) => set({ enabled: event.target.checked })}
          />
          有効
        </label>
      </div>
      <p className="panel__muted small">トラックの「リバーブ送り」で各楽器の残響量を調整できます。</p>
      <div className="grid2">
        {REVERB_FIELDS.map((field) => (
          <label key={field.key} className="field">
            <span className="field__label">
              {field.label} <em>{field.display(reverb[field.key])}</em>
            </span>
            <input
              className="range"
              type="range"
              aria-label={field.label}
              min={field.min}
              max={field.max}
              step={field.step}
              value={reverb[field.key]}
              onChange={(event) => set({ [field.key]: Number(event.target.value) })}
            />
          </label>
        ))}
      </div>
    </section>
  );
}
