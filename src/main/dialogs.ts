import { type BrowserWindow, dialog, type OpenDialogOptions, type SaveDialogOptions } from "electron";
import { readFile } from "node:fs/promises";
import { basename } from "node:path";
import type { BounceRequest, BounceResponse, LoadedFile } from "../shared/media";
import { type ExportRequest, writeExport } from "./media/encode";

const AUDIO_EXTENSIONS = ["wav", "mp3", "ogg", "flac", "m4a", "aac", "aif", "aiff", "opus", "wma"];

function showOpen(window: BrowserWindow | null, options: OpenDialogOptions): ReturnType<typeof dialog.showOpenDialog> {
  return window === null ? dialog.showOpenDialog(options) : dialog.showOpenDialog(window, options);
}

function showSave(window: BrowserWindow | null, options: SaveDialogOptions): ReturnType<typeof dialog.showSaveDialog> {
  return window === null ? dialog.showSaveDialog(options) : dialog.showSaveDialog(window, options);
}

async function toLoadedFile(filePath: string): Promise<LoadedFile> {
  const data = await readFile(filePath);
  return { name: basename(filePath), data: new Uint8Array(data) };
}

export async function openMidi(window: BrowserWindow | null): Promise<LoadedFile | null> {
  const result = await showOpen(window, {
    title: "MIDI ファイルを開く",
    properties: ["openFile"],
    filters: [{ name: "MIDI", extensions: ["mid", "midi"] }],
  });
  const filePath = result.filePaths[0];
  if (result.canceled || filePath === undefined) {
    return null;
  }
  return toLoadedFile(filePath);
}

export async function openAudio(window: BrowserWindow | null): Promise<LoadedFile[] | null> {
  const result = await showOpen(window, {
    title: "音声素材を追加",
    properties: ["openFile", "multiSelections"],
    filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
  });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return Promise.all(result.filePaths.map(toLoadedFile));
}

export async function bounce(window: BrowserWindow | null, request: BounceRequest): Promise<BounceResponse> {
  const ext = request.format;
  const suggested = request.defaultName.toLowerCase().endsWith(`.${ext}`)
    ? request.defaultName
    : `${request.defaultName}.${ext}`;
  const save = await showSave(window, {
    title: "音MAD を書き出す",
    defaultPath: suggested,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (save.canceled || save.filePath === undefined) {
    return { ok: false, canceled: true };
  }
  try {
    const options: ExportRequest = { format: request.format, path: save.filePath };
    if (request.wavBitDepth !== undefined) {
      options.wavBitDepth = request.wavBitDepth;
    }
    if (request.mp3Bitrate !== undefined) {
      options.mp3Bitrate = request.mp3Bitrate;
    }
    const result = await writeExport(request.pcm, options);
    return { ok: true, ...result };
  } catch (error) {
    return { ok: false, canceled: false, error: error instanceof Error ? error.message : String(error) };
  }
}
