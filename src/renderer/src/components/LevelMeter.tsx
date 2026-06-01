import { useEffect, useRef, useState } from "react";
import { useStudio } from "../state/StudioContext";

export function LevelMeter(): React.JSX.Element {
  const { engineRef } = useStudio();
  const [level, setLevel] = useState(0);
  const peakRef = useRef(0);

  useEffect(() => {
    let raf = 0;
    let buffer: Float32Array<ArrayBuffer> | null = null;
    const tick = (): void => {
      const engine = engineRef.current;
      if (engine !== null) {
        const analyser = engine.getMasterAnalyser();
        if (buffer === null || buffer.length !== analyser.fftSize) {
          buffer = new Float32Array(analyser.fftSize);
        }
        analyser.getFloatTimeDomainData(buffer);
        let sum = 0;
        for (let i = 0; i < buffer.length; i += 1) {
          const v = buffer[i]!;
          sum += v * v;
        }
        const rms = Math.sqrt(sum / buffer.length);
        peakRef.current = Math.max(rms, peakRef.current * 0.9);
        setLevel(Math.min(1, peakRef.current * 1.6));
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [engineRef]);

  return (
    <div className="meter" title="マスターレベル">
      <div className="meter__fill" style={{ width: `${level * 100}%` }} />
    </div>
  );
}
