import { useEffect, useRef } from "react";
import type { Track } from "../../../shared/schemas/project";
import { useStudio } from "../state/StudioContext";

interface TrackRowProps {
  track: Track;
  pxPerSec: number;
  rowHeight: number;
  canvasWidth: number;
  selected: boolean;
}

const HEADER_WIDTH = 200;

export function TrackRow({ track, pxPerSec, rowHeight, canvasWidth, selected }: TrackRowProps): React.JSX.Element {
  const { project, selectTrack, updateTrack, seek } = useStudio();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) {
      return;
    }
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      return;
    }
    canvas.width = canvasWidth;
    canvas.height = rowHeight;
    ctx.clearRect(0, 0, canvasWidth, rowHeight);

    if (track.notes.length === 0) {
      return;
    }
    let minPitch = 127;
    let maxPitch = 0;
    for (const note of track.notes) {
      minPitch = Math.min(minPitch, note.pitch);
      maxPitch = Math.max(maxPitch, note.pitch);
    }
    minPitch -= 1;
    maxPitch += 1;
    const range = Math.max(1, maxPitch - minPitch + 1);
    const noteHeight = rowHeight / range;

    for (let p = minPitch; p <= maxPitch; p += 1) {
      if (p % 12 === 0) {
        const y = (maxPitch - p) * noteHeight;
        ctx.fillStyle = "rgba(255,255,255,0.04)";
        ctx.fillRect(0, y, canvasWidth, noteHeight);
      }
    }

    for (const note of track.notes) {
      const x = note.startSec * pxPerSec;
      if (x > canvasWidth) {
        continue;
      }
      const w = Math.max(2, note.durationSec * pxPerSec);
      const y = (maxPitch - note.pitch) * noteHeight;
      const h = Math.max(2, noteHeight - 1);
      const override = track.noteSampleMap[String(note.pitch)] !== undefined;
      const alpha = 0.4 + 0.6 * (note.velocity / 127);
      ctx.fillStyle = override ? "#ffd34d" : track.color;
      ctx.globalAlpha = alpha;
      ctx.fillRect(x, y, w, h);
      ctx.globalAlpha = 1;
    }
  }, [track.notes, track.color, track.noteSampleMap, pxPerSec, rowHeight, canvasWidth]);

  const someSolo = project.tracks.some((item) => item.solo);
  const dimmed = track.muted || (someSolo && !track.solo);

  return (
    <div className={`trackrow${selected ? " trackrow--selected" : ""}`} style={{ height: rowHeight }}>
      <div className="trackrow__header" style={{ width: HEADER_WIDTH }}>
        <button type="button" className="trackrow__name" onClick={() => selectTrack(track.id)} title="トラックを選択">
          <span className="trackrow__swatch" style={{ background: track.color }} />
          <span className="trackrow__label">{track.name}</span>
        </button>
        <div className="trackrow__controls">
          <button
            type="button"
            className={`tag${track.muted ? " tag--on" : ""}`}
            onClick={() => updateTrack(track.id, { muted: !track.muted })}
          >
            M
          </button>
          <button
            type="button"
            className={`tag${track.solo ? " tag--solo" : ""}`}
            onClick={() => updateTrack(track.id, { solo: !track.solo })}
          >
            S
          </button>
          <select
            className="select select--mini"
            value={track.defaultSampleId ?? ""}
            onChange={(event) =>
              updateTrack(track.id, { defaultSampleId: event.target.value === "" ? null : event.target.value })
            }
          >
            <option value="">（素材なし）</option>
            {project.samples.map((sample) => (
              <option key={sample.id} value={sample.id}>
                {sample.name}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div
        className={`trackrow__lane${dimmed ? " trackrow__lane--dim" : ""}`}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          seek((event.clientX - rect.left) / pxPerSec);
        }}
      >
        <canvas ref={canvasRef} />
      </div>
    </div>
  );
}
