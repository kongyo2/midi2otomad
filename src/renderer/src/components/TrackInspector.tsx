import { useMemo } from "react";
import { useStudio } from "../state/StudioContext";
import { midiToNoteName } from "../../../shared/music/pitch";
import type { StopMode, VoicePriority } from "../../../shared/schemas/project";
import { formatDb } from "../util/format";

const PRIORITY_LABELS: Record<VoicePriority, string> = {
  newest: "新しい音を優先",
  oldest: "古い音を優先",
  highest: "高い音を優先",
  lowest: "低い音を優先",
};

const STOP_MODE_LABELS: Record<StopMode, string> = {
  none: "重ねる（停止しない）",
  pitch: "同じ音程を停止",
  sample: "同じ素材を停止",
  track: "トラック全体を停止",
};

export function TrackInspector(): React.JSX.Element {
  const { project, selectedTrackId, updateTrack, setNoteSample, selectSample } = useStudio();
  const track = project.tracks.find((item) => item.id === selectedTrackId);

  const distinctPitches = useMemo(() => {
    if (track === undefined) {
      return [];
    }
    const set = new Set<number>();
    for (const note of track.notes) {
      set.add(note.pitch);
    }
    return [...set].sort((a, b) => a - b);
  }, [track]);

  if (track === undefined) {
    return (
      <section className="panel">
        <h2 className="panel__heading">トラック設定</h2>
        <p className="panel__muted">
          タイムラインのトラック名をクリックすると、音量・パン・素材割り当てを編集できます。
        </p>
      </section>
    );
  }

  const dynamics = track.dynamics;
  const hasExpression = dynamics.expression.length > 0 || dynamics.volume.length > 0;
  const dynamicsHint = !dynamics.enabled
    ? "🎚 抑揚をオフにして、各ノートのベロシティのみで音量を決めます。"
    : hasExpression
      ? "🎚 ベロシティ＋エクスプレッション(CC11)/ボリューム(CC7) を音量に反映します。"
      : "🎚 各ノートのベロシティを音量に反映します。";

  return (
    <section className="panel">
      <div className="panel__head">
        <h2 className="panel__heading">トラック設定</h2>
        <span className="pill" style={{ background: track.color }}>
          {track.notes.length} ノート
        </span>
      </div>

      <label className="field">
        <span className="field__label">名前</span>
        <input
          className="input"
          value={track.name}
          onChange={(event) => updateTrack(track.id, { name: event.target.value })}
        />
      </label>

      <label className="field">
        <span className="field__label">既定の音声素材</span>
        <select
          className="select"
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
      </label>

      <div className="grid2">
        <label className="field">
          <span className="field__label">
            音量 <em>{formatDb(track.gain)}</em>
          </span>
          <input
            className="range"
            type="range"
            min={0}
            max={4}
            step={0.01}
            value={track.gain}
            onChange={(event) => updateTrack(track.id, { gain: Number(event.target.value) })}
          />
        </label>
        <label className="field">
          <span className="field__label">
            パン{" "}
            <em>
              {track.pan === 0
                ? "C"
                : track.pan < 0
                  ? `L${Math.round(-track.pan * 100)}`
                  : `R${Math.round(track.pan * 100)}`}
            </em>
          </span>
          <input
            className="range"
            type="range"
            min={-1}
            max={1}
            step={0.01}
            value={track.pan}
            onChange={(event) => updateTrack(track.id, { pan: Number(event.target.value) })}
          />
        </label>
      </div>

      <label className="field">
        <span className="field__label">
          リバーブ送り <em>{Math.round(track.reverbSend * 100)}%</em>
        </span>
        <input
          className="range"
          type="range"
          aria-label="リバーブ送り"
          min={0}
          max={1}
          step={0.01}
          value={track.reverbSend}
          onChange={(event) => updateTrack(track.id, { reverbSend: Number(event.target.value) })}
        />
      </label>

      <h3 className="subheading">抑揚（ダイナミクス）</h3>
      <label className="checkline">
        <input
          type="checkbox"
          aria-label="抑揚を反映"
          checked={dynamics.enabled}
          onChange={(event) => updateTrack(track.id, { dynamics: { ...dynamics, enabled: event.target.checked } })}
        />
        ベロシティ・エクスプレッションを反映
      </label>
      <label className="field">
        <span className="field__label">
          抑揚の深さ <em>{Math.round(dynamics.amount * 100)}%</em>
        </span>
        <input
          className="range"
          type="range"
          aria-label="抑揚の深さ"
          min={0}
          max={1}
          step={0.01}
          value={dynamics.amount}
          onChange={(event) => updateTrack(track.id, { dynamics: { ...dynamics, amount: Number(event.target.value) } })}
        />
      </label>
      <p className="hintline">{dynamicsHint}</p>

      <h3 className="subheading">ボイス（同時発音）管理</h3>
      <label className="field">
        <span className="field__label">
          最大同時発音数 <em>{track.polyphony.maxVoices === 0 ? "無制限" : `${track.polyphony.maxVoices} 音`}</em>
        </span>
        <input
          className="input"
          type="number"
          min={0}
          max={64}
          step={1}
          value={track.polyphony.maxVoices}
          onChange={(event) =>
            updateTrack(track.id, {
              polyphony: {
                ...track.polyphony,
                maxVoices: Math.max(0, Math.min(64, Math.round(Number(event.target.value)))),
              },
            })
          }
        />
      </label>

      <div className="grid2">
        <label className="field">
          <span className="field__label">優先再生</span>
          <select
            className="select"
            value={track.polyphony.priority}
            onChange={(event) =>
              updateTrack(track.id, {
                polyphony: { ...track.polyphony, priority: event.target.value as VoicePriority },
              })
            }
          >
            {Object.entries(PRIORITY_LABELS).map(([key, label]) => (
              <option key={key} value={key}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">停止方法</span>
          <select
            className="select"
            value={track.polyphony.stopMode}
            onChange={(event) =>
              updateTrack(track.id, {
                polyphony: { ...track.polyphony, stopMode: event.target.value as StopMode },
              })
            }
          >
            {Object.entries(STOP_MODE_LABELS).map(([key, label]) => (
              <option key={key} value={key}>
                {label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className="hintline">🎹 同時に鳴らせる音数・あふれた時に残す音・新しい音で止める範囲を設定します。</p>

      <h3 className="subheading">ノート番号ごとの素材割り当て</h3>
      <p className="panel__muted small">特定の音だけ別素材に差し替えできます（ドラムキットや音域別の貼り替えに）。</p>
      <div className="notemap">
        {distinctPitches.length === 0 ? (
          <p className="panel__muted">ノートがありません。</p>
        ) : (
          distinctPitches.map((pitch) => {
            const assigned = track.noteSampleMap[String(pitch)] ?? "";
            return (
              <div key={pitch} className={`notemap__row${assigned !== "" ? " notemap__row--override" : ""}`}>
                <span className="notemap__pitch">{midiToNoteName(pitch)}</span>
                <select
                  className="select select--mini"
                  value={assigned}
                  onChange={(event) => {
                    const value = event.target.value;
                    setNoteSample(track.id, pitch, value === "" ? null : value);
                    if (value !== "") {
                      selectSample(value);
                    }
                  }}
                >
                  <option value="">（既定）</option>
                  {project.samples.map((sample) => (
                    <option key={sample.id} value={sample.id}>
                      {sample.name}
                    </option>
                  ))}
                </select>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
