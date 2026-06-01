import { useEffect, useRef, useState } from "react";
import { useStudio } from "./state/StudioContext";
import { TopBar } from "./components/TopBar";
import { SampleLibrary } from "./components/SampleLibrary";
import { SampleInspector } from "./components/SampleInspector";
import { Timeline } from "./components/Timeline";
import { TrackInspector } from "./components/TrackInspector";
import { HelpPanel } from "./components/HelpPanel";

export function App(): React.JSX.Element {
  const { importMidiBytes, ingestAudio, togglePlay, toast, busy } = useStudio();
  const [dragActive, setDragActive] = useState(false);
  const dragDepth = useRef(0);

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const target = event.target;
      if (target instanceof HTMLElement && ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName)) {
        return;
      }
      if (event.code === "Space") {
        event.preventDefault();
        togglePlay();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePlay]);

  const handleDrop = async (event: React.DragEvent): Promise<void> => {
    event.preventDefault();
    dragDepth.current = 0;
    setDragActive(false);
    const files = Array.from(event.dataTransfer.files);
    const midiFiles = files.filter((file) => /\.midi?$/i.test(file.name));
    const audioFiles = files.filter((file) => !/\.midi?$/i.test(file.name));
    const midiBuffers = await Promise.all(
      midiFiles.map(async (midi) => ({ name: midi.name, bytes: new Uint8Array(await midi.arrayBuffer()) })),
    );
    for (const midi of midiBuffers) {
      importMidiBytes(midi.bytes, midi.name);
    }
    if (audioFiles.length > 0) {
      await ingestAudio(audioFiles);
    }
  };

  return (
    <div
      className="studio"
      onDragEnter={(event) => {
        event.preventDefault();
        dragDepth.current += 1;
        setDragActive(true);
      }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={() => {
        dragDepth.current -= 1;
        if (dragDepth.current <= 0) {
          setDragActive(false);
        }
      }}
      onDrop={(event) => void handleDrop(event)}
    >
      <TopBar />
      <div className="studio__body">
        <aside className="studio__left">
          <SampleLibrary />
          <SampleInspector />
        </aside>
        <main className="studio__center">
          <Timeline />
        </main>
        <aside className="studio__right">
          <TrackInspector />
          <HelpPanel />
        </aside>
      </div>

      {dragActive ? (
        <div className="dropzone-overlay">
          <div className="dropzone-overlay__card">
            <div className="dropzone-overlay__icon">🎼</div>
            <p className="dropzone-overlay__title">ここにドロップ</p>
            <p className="dropzone-overlay__sub">.mid → アレンジ読込 ／ wav・mp3 → 音声素材として追加</p>
          </div>
        </div>
      ) : null}

      {busy !== null ? (
        <div className="busybar">
          <span className="busybar__spinner" /> {busy}
        </div>
      ) : null}

      {toast !== null ? <div className="toast">{toast}</div> : null}
    </div>
  );
}
