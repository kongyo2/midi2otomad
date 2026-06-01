import { useState } from "react";
import { useStudio } from "../state/StudioContext";
import { midiToNoteName } from "../../../shared/music/pitch";

export function SampleLibrary(): React.JSX.Element {
  const { project, selectedSampleId, selectSample, removeSample, ingestAudio, getPeaks } = useStudio();
  const [dragOver, setDragOver] = useState(false);

  const onAdd = async (): Promise<void> => {
    const files = await window.api.openAudio();
    if (files !== null) {
      await ingestAudio(files);
    }
  };

  return (
    <section className="panel">
      <div className="panel__head">
        <h2 className="panel__heading">音声素材ライブラリ</h2>
        <button type="button" className="btn btn--sm" onClick={() => void onAdd()}>
          + 追加
        </button>
      </div>

      <div
        className={`droparea${dragOver ? " droparea--over" : ""}`}
        onDragOver={(event) => {
          event.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(event) => {
          event.preventDefault();
          event.stopPropagation();
          setDragOver(false);
          const files = Array.from(event.dataTransfer.files).filter((file) => !/\.midi?$/i.test(file.name));
          if (files.length > 0) {
            void ingestAudio(files);
          }
        }}
      >
        {project.samples.length === 0 ? (
          <p className="droparea__hint">WAV / MP3 などをここにドロップ、または「追加」</p>
        ) : (
          <ul className="samplelist">
            {project.samples.map((sample) => {
              const peaks = getPeaks(sample.id);
              const selected = sample.id === selectedSampleId;
              return (
                <li key={sample.id}>
                  <button
                    type="button"
                    className={`samplelist__item${selected ? " samplelist__item--active" : ""}`}
                    onClick={() => selectSample(sample.id)}
                  >
                    <MiniWave peaks={peaks} />
                    <span className="samplelist__meta">
                      <span className="samplelist__name">{sample.name}</span>
                      <span className="samplelist__sub">
                        基準 {midiToNoteName(sample.basePitch)} · {sample.durationSec.toFixed(2)}s
                        {sample.loop.enabled ? " · ⟳loop" : ""}
                      </span>
                    </span>
                  </button>
                  <button type="button" className="iconbtn" title="削除" onClick={() => removeSample(sample.id)}>
                    ✕
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}

function MiniWave({ peaks }: { peaks: Float32Array | undefined }): React.JSX.Element {
  const bars: number[] = [];
  if (peaks !== undefined) {
    const count = 22;
    const step = Math.max(1, Math.floor(peaks.length / count));
    for (let i = 0; i < count; i += 1) {
      bars.push(peaks[i * step] ?? 0);
    }
  }
  return (
    <span className="miniwave" aria-hidden="true">
      {bars.map((value, index) => (
        <span key={index} className="miniwave__bar" style={{ height: `${Math.max(8, value * 100)}%` }} />
      ))}
    </span>
  );
}
