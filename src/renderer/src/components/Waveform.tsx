import { useEffect, useRef } from "react";

interface WaveformProps {
  peaks: Float32Array | undefined;
  height?: number;
  loop?: { startFrac: number; endFrac: number; enabled: boolean } | undefined;
  color?: string;
}

export function Waveform({ peaks, height = 96, loop, color = "#7c5cff" }: WaveformProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) {
      return;
    }
    const draw = (): void => {
      const ctx = canvas.getContext("2d");
      const parent = canvas.parentElement;
      if (ctx === null || parent === null) {
        return;
      }
      const dpr = window.devicePixelRatio || 1;
      const width = parent.clientWidth;
      canvas.width = Math.max(1, Math.floor(width * dpr));
      canvas.height = Math.floor(height * dpr);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, width, height);

      ctx.fillStyle = "rgba(255,255,255,0.03)";
      ctx.fillRect(0, 0, width, height);

      if (loop?.enabled === true) {
        const x0 = loop.startFrac * width;
        const x1 = loop.endFrac * width;
        ctx.fillStyle = "rgba(124,92,255,0.18)";
        ctx.fillRect(x0, 0, Math.max(1, x1 - x0), height);
        ctx.strokeStyle = "rgba(124,92,255,0.9)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(x0, 0);
        ctx.lineTo(x0, height);
        ctx.moveTo(x1, 0);
        ctx.lineTo(x1, height);
        ctx.stroke();
      }

      const mid = height / 2;
      ctx.strokeStyle = "rgba(255,255,255,0.12)";
      ctx.beginPath();
      ctx.moveTo(0, mid);
      ctx.lineTo(width, mid);
      ctx.stroke();

      if (peaks === undefined || peaks.length === 0) {
        return;
      }
      ctx.fillStyle = color;
      const barCount = peaks.length;
      const barWidth = width / barCount;
      for (let i = 0; i < barCount; i += 1) {
        const value = peaks[i] ?? 0;
        const h = Math.max(1, value * (height - 6));
        ctx.fillRect(i * barWidth, mid - h / 2, Math.max(0.6, barWidth - 0.3), h);
      }
    };

    draw();
    window.addEventListener("resize", draw);
    return () => window.removeEventListener("resize", draw);
  }, [peaks, height, loop, color]);

  return (
    <div className="waveform">
      <canvas ref={canvasRef} />
    </div>
  );
}
