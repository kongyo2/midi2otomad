import { useStudio } from "../state/StudioContext";
import type { Output } from "../../../shared/schemas/project";
import { formatDb } from "../util/format";

const SAMPLE_RATES = [44100, 48000, 88200, 96000] as const;

const formatRate = (hz: number): string => `${(hz / 1000).toFixed(hz % 1000 === 0 ? 0 : 1)} kHz`;

export function OutputPanel(): React.JSX.Element {
  const { project, patchProject } = useStudio();
  const output = project.output;
  const setOutput = (patch: Partial<Output>): void => patchProject({ output: { ...output, ...patch } });

  return (
    <section className="panel">
      <h2 className="panel__heading">出力設定</h2>
      <p className="panel__muted small">書き出しの解像度・余韻と、仕上げのマスターリミッターをまとめて調整します。</p>

      <div className="grid2">
        <label className="field">
          <span className="field__label">サンプルレート</span>
          <select
            className="select"
            aria-label="サンプルレート"
            value={project.sampleRate}
            onChange={(event) => patchProject({ sampleRate: Number(event.target.value) })}
          >
            {SAMPLE_RATES.map((hz) => (
              <option key={hz} value={hz}>
                {formatRate(hz)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">
            テール <em>{output.tailSec.toFixed(2)} s</em>
          </span>
          <input
            className="range"
            type="range"
            aria-label="テール"
            min={0}
            max={10}
            step={0.05}
            value={output.tailSec}
            onChange={(event) => setOutput({ tailSec: Number(event.target.value) })}
          />
        </label>
      </div>

      <div className="panel__head">
        <h3 className="subheading">マスターリミッター</h3>
        <label className="checkline">
          <input
            type="checkbox"
            aria-label="リミッター"
            checked={output.limiter.enabled}
            onChange={(event) => setOutput({ limiter: { ...output.limiter, enabled: event.target.checked } })}
          />
          有効
        </label>
      </div>
      <label className="field">
        <span className="field__label">
          スレッショルド <em>{formatDb(output.limiter.threshold)}</em>
        </span>
        <input
          className="range"
          type="range"
          aria-label="スレッショルド"
          min={0.1}
          max={1}
          step={0.01}
          value={output.limiter.threshold}
          onChange={(event) => setOutput({ limiter: { ...output.limiter, threshold: Number(event.target.value) } })}
        />
      </label>
    </section>
  );
}
