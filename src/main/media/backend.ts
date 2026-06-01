import type { MediaProbe } from "../../shared/media";

type NodeAv = typeof import("node-av");

let nodeAvPromise: Promise<NodeAv> | null = null;

function loadNodeAv(): Promise<NodeAv> {
  return (nodeAvPromise ??= import("node-av"));
}

export async function probeMedia(): Promise<MediaProbe> {
  const av = await loadNodeAv();
  return {
    backend: "node-av",
    ffmpegVersion: av.ffmpegVersion(),
  };
}
