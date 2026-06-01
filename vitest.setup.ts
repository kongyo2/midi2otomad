import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

afterEach(() => {
  cleanup();
});

interface Mock2dContext {
  scale: () => void;
  clearRect: () => void;
  fillRect: () => void;
  beginPath: () => void;
  moveTo: () => void;
  lineTo: () => void;
  stroke: () => void;
  fillStyle: string;
  strokeStyle: string;
  lineWidth: number;
  globalAlpha: number;
}

function createMock2dContext(): Mock2dContext {
  return {
    scale: () => undefined,
    clearRect: () => undefined,
    fillRect: () => undefined,
    beginPath: () => undefined,
    moveTo: () => undefined,
    lineTo: () => undefined,
    stroke: () => undefined,
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 0,
    globalAlpha: 1,
  };
}

if (typeof HTMLCanvasElement !== "undefined") {
  HTMLCanvasElement.prototype.getContext = function getContext(): Mock2dContext {
    return createMock2dContext();
  } as unknown as typeof HTMLCanvasElement.prototype.getContext;
}

// jsdom may provide a timer-backed rAF; force it inert so render loops never run
// on their own. Tests that exercise a frame capture the callback and invoke it.
globalThis.requestAnimationFrame = (() => 0) as typeof globalThis.requestAnimationFrame;
globalThis.cancelAnimationFrame = (() => undefined) as typeof globalThis.cancelAnimationFrame;
