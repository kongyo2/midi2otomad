import { useEffect, useState } from "react";
import type { Project } from "../../shared/schemas/project";
import type { MediaProbe } from "../../shared/media";

export function App() {
  const [electronVersion, setElectronVersion] = useState("…");
  const [project, setProject] = useState<Project | null>(null);
  const [pong, setPong] = useState("");
  const [media, setMedia] = useState<MediaProbe | null>(null);

  useEffect(() => {
    void window.api.getVersion().then(setElectronVersion);
    void window.api.defaultProject().then(setProject);
    void window.api.probeMedia().then(setMedia);
  }, []);

  return (
    <div className="app">
      <header className="app__header">
        <div className="app__brand">
          <span className="app__logo">🎹</span>
          <h1 className="app__title">midi2otomad</h1>
        </div>
        <span className="app__badge">Electron {electronVersion}</span>
      </header>

      <main className="app__main">
        <section className="panel">
          <h2 className="panel__heading">プロジェクト</h2>
          {project === null ? (
            <p className="panel__muted">読み込み中…</p>
          ) : (
            <dl className="kv">
              <div className="kv__row">
                <dt>名前</dt>
                <dd>{project.name}</dd>
              </div>
              <div className="kv__row">
                <dt>BPM</dt>
                <dd>{project.bpm}</dd>
              </div>
              <div className="kv__row">
                <dt>PPQ</dt>
                <dd>{project.ppq}</dd>
              </div>
              <div className="kv__row">
                <dt>トラック数</dt>
                <dd>{project.tracks.length}</dd>
              </div>
            </dl>
          )}
        </section>

        <section className="panel">
          <h2 className="panel__heading">IPC 疎通</h2>
          <button
            type="button"
            className="btn"
            onClick={() => {
              void window.api.ping().then(setPong);
            }}
          >
            ping を送信
          </button>
          {pong !== "" && <p className="panel__ok">main 応答: {pong}</p>}
        </section>

        <section className="panel">
          <h2 className="panel__heading">メディアバックエンド</h2>
          {media === null ? (
            <p className="panel__muted">初期化中…</p>
          ) : (
            <dl className="kv">
              <div className="kv__row">
                <dt>バックエンド</dt>
                <dd>{media.backend}</dd>
              </div>
              <div className="kv__row">
                <dt>FFmpeg</dt>
                <dd>{media.ffmpegVersion}</dd>
              </div>
            </dl>
          )}
        </section>

        <p className="app__note">
          初期セットアップ完了。ここに今後、ピアノロール / タイムライン / サンプル管理 UI を実装していきます。
        </p>
      </main>
    </div>
  );
}
