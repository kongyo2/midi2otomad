import { useEffect, useMemo, useRef, useState } from "react";
import { useStudio, usePlayhead } from "../state/StudioContext";
import { TrackRow } from "./TrackRow";
import { formatTime } from "../util/format";

const HEADER_WIDTH = 200;
const ROW_HEIGHT = 96;
const RULER_HEIGHT = 30;
const MAX_CANVAS_WIDTH = 30000;

export function Timeline(): React.JSX.Element {
  const { project, selectedTrackId, seek, isPlaying } = useStudio();
  const playhead = usePlayhead();
  const [pxRequested, setPxRequested] = useState(80);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const duration = useMemo(() => {
    let end = 0;
    for (const track of project.tracks) {
      for (const note of track.notes) {
        end = Math.max(end, note.startSec + note.durationSec);
      }
    }
    return Math.max(end + 2, 8);
  }, [project.tracks]);

  // Canvas elements have a hard pixel-width limit, so cap the effective zoom to keep
  // the whole song within one canvas. Every position (notes, playhead, seek) uses this
  // same scale so nothing becomes unreachable on long arrangements.
  const pxPerSec = Math.min(pxRequested, MAX_CANVAS_WIDTH / Math.max(1, duration));
  const canvasWidth = Math.min(MAX_CANVAS_WIDTH, Math.ceil(duration * pxPerSec));
  const contentWidth = HEADER_WIDTH + canvasWidth;
  const playheadX = HEADER_WIDTH + playhead * pxPerSec;

  useEffect(() => {
    if (!isPlaying) {
      return;
    }
    const scroller = scrollRef.current!;
    const viewLeft = scroller.scrollLeft;
    const viewRight = viewLeft + scroller.clientWidth;
    if (playheadX < viewLeft + HEADER_WIDTH || playheadX > viewRight - 80) {
      scroller.scrollLeft = Math.max(0, playheadX - scroller.clientWidth * 0.4);
    }
  }, [playheadX, isPlaying]);

  const ticks = useMemo(() => {
    const result: number[] = [];
    const step = pxPerSec < 50 ? 5 : pxPerSec < 100 ? 2 : 1;
    for (let t = 0; t <= duration; t += step) {
      result.push(t);
    }
    return result;
  }, [duration, pxPerSec]);

  return (
    <section className="timeline-panel">
      <div className="timeline-toolbar">
        <span className="timeline-toolbar__title">タイムライン / ピアノロール</span>
        <div className="timeline-toolbar__zoom">
          <button type="button" className="iconbtn" onClick={() => setPxRequested((v) => Math.max(24, v - 16))}>
            －
          </button>
          <span className="zoomlabel">{Math.round(pxPerSec)}px/s</span>
          <button type="button" className="iconbtn" onClick={() => setPxRequested((v) => Math.min(200, v + 16))}>
            ＋
          </button>
        </div>
      </div>

      <div className="timeline" ref={scrollRef}>
        <div className="timeline__content" style={{ width: contentWidth }}>
          <div className="timeline__rulerrow" style={{ height: RULER_HEIGHT }}>
            <div className="timeline__corner" style={{ width: HEADER_WIDTH }}>
              {formatTime(playhead)}
            </div>
            <div
              className="ruler"
              style={{ width: canvasWidth }}
              onClick={(event) => {
                const rect = event.currentTarget.getBoundingClientRect();
                seek((event.clientX - rect.left) / pxPerSec);
              }}
            >
              {ticks.map((t) => (
                <span key={t} className="ruler__tick" style={{ left: t * pxPerSec }}>
                  {t}s
                </span>
              ))}
            </div>
          </div>

          {project.tracks.length === 0 ? (
            <div className="timeline__empty">
              <p>
                <strong>.mid</strong> ファイルをドラッグ＆ドロップして始めましょう。
              </p>
              <p className="panel__muted">トラック・ノート・テンポを解析し、ここにピアノロールを表示します。</p>
            </div>
          ) : (
            project.tracks.map((track) => (
              <TrackRow
                key={track.id}
                track={track}
                pxPerSec={pxPerSec}
                rowHeight={ROW_HEIGHT}
                canvasWidth={canvasWidth}
                selected={track.id === selectedTrackId}
              />
            ))
          )}

          {project.tracks.length > 0 ? (
            <div className="playhead" style={{ left: playheadX }} aria-hidden="true" />
          ) : null}
        </div>
      </div>
    </section>
  );
}
