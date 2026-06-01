import { afterEach, describe, expect, it, vi } from "vitest";
import { IPC } from "../shared/ipc";

type AnyFn = (...args: unknown[]) => unknown;

const h = vi.hoisted(() => {
  const handlers: Record<string, AnyFn> = {};
  const appOn: Record<string, AnyFn> = {};
  const windowEvents: Record<string, AnyFn> = {};
  const store = {
    windowOpenHandler: null as null | ((details: { url: string }) => unknown),
    headersCb: null as
      | null
      | ((details: { responseHeaders: Record<string, unknown> }, cb: (r: unknown) => void) => void),
  };
  const state = { whenReadyRejects: false, allWindows: [] as unknown[] };

  const showSpy = vi.fn();
  const loadURL = vi.fn();
  const loadFile = vi.fn();
  const winOn = vi.fn((event: string, cb: AnyFn) => {
    windowEvents[event] = cb;
  });
  const setWindowOpenHandler = vi.fn((fn: (details: { url: string }) => unknown) => {
    store.windowOpenHandler = fn;
  });
  const BrowserWindow = vi.fn(function BrowserWindowMock() {
    return {
      on: winOn,
      webContents: { setWindowOpenHandler },
      loadURL,
      loadFile,
      show: showSpy,
    };
  });
  const getAllWindows = vi.fn(() => state.allWindows);
  (BrowserWindow as unknown as { getAllWindows: typeof getAllWindows }).getAllWindows = getAllWindows;

  const whenReady = vi.fn(() => (state.whenReadyRejects ? Promise.reject(new Error("not ready")) : Promise.resolve()));
  const appOnFn = vi.fn((event: string, cb: AnyFn) => {
    appOn[event] = cb;
  });
  const quit = vi.fn();
  const app = { whenReady, on: appOnFn, quit };

  const handle = vi.fn((channel: string, cb: AnyFn) => {
    handlers[channel] = cb;
  });
  const ipcMain = { handle };

  const onHeadersReceived = vi.fn((cb: typeof store.headersCb) => {
    store.headersCb = cb;
  });
  const session = { defaultSession: { webRequest: { onHeadersReceived } } };

  const openExternal = vi.fn(() => Promise.resolve());
  const shell = { openExternal };

  const probeMedia = vi.fn(async () => ({ backend: "node-av", ffmpegVersion: "8.1" }));
  const bounce = vi.fn(async () => ({ ok: true, path: "/x", bytes: 1, durationSec: 1 }));
  const openMidi = vi.fn(async () => null);
  const openAudio = vi.fn(async () => null);

  return {
    handlers,
    appOn,
    windowEvents,
    store,
    state,
    showSpy,
    loadURL,
    loadFile,
    BrowserWindow,
    getAllWindows,
    app,
    ipcMain,
    handle,
    onHeadersReceived,
    session,
    shell,
    openExternal,
    probeMedia,
    bounce,
    openMidi,
    openAudio,
  };
});

vi.mock("electron", () => ({
  app: h.app,
  BrowserWindow: h.BrowserWindow,
  ipcMain: h.ipcMain,
  session: h.session,
  shell: h.shell,
}));
vi.mock("./media/backend", () => ({ probeMedia: h.probeMedia }));
vi.mock("./dialogs", () => ({ bounce: h.bounce, openMidi: h.openMidi, openAudio: h.openAudio }));

const originalPlatform = Object.getOwnPropertyDescriptor(process, "platform")!;

function setPlatform(value: string): void {
  Object.defineProperty(process, "platform", { value, configurable: true });
}

async function loadMain(opts: { rendererUrl?: string } = {}): Promise<void> {
  vi.resetModules();
  vi.clearAllMocks();
  for (const key of Object.keys(h.handlers)) delete h.handlers[key];
  for (const key of Object.keys(h.appOn)) delete h.appOn[key];
  for (const key of Object.keys(h.windowEvents)) delete h.windowEvents[key];
  h.store.windowOpenHandler = null;
  h.store.headersCb = null;
  if (opts.rendererUrl === undefined) {
    delete process.env["ELECTRON_RENDERER_URL"];
  } else {
    process.env["ELECTRON_RENDERER_URL"] = opts.rendererUrl;
  }
  await import("./index");
  await new Promise((resolve) => setTimeout(resolve, 0));
}

afterEach(() => {
  Object.defineProperty(process, "platform", originalPlatform);
  delete process.env["ELECTRON_RENDERER_URL"];
  h.state.allWindows = [];
  h.state.whenReadyRejects = false;
});

describe("main process bootstrap", () => {
  it("registers every IPC handler and applies the production CSP", async () => {
    await loadMain();

    expect(h.handle).toHaveBeenCalledTimes(7);
    expect(h.handlers[IPC.ping]!()).toBe("pong");
    expect(h.handlers[IPC.getVersion]!()).toBe(process.versions.electron);
    expect(h.handlers[IPC.defaultProject]!()).toMatchObject({ version: 1, name: "Untitled 音MAD" });

    await h.handlers[IPC.probeMedia]!();
    expect(h.probeMedia).toHaveBeenCalledTimes(1);
    await h.handlers[IPC.openMidi]!();
    expect(h.openMidi).toHaveBeenCalledTimes(1);
    await h.handlers[IPC.openAudio]!();
    expect(h.openAudio).toHaveBeenCalledTimes(1);
    const request = { format: "wav", defaultName: "song" };
    await h.handlers[IPC.bounce]!({}, request);
    expect(h.bounce).toHaveBeenCalledWith(expect.anything(), request);

    expect(h.onHeadersReceived).toHaveBeenCalledTimes(1);
    const callback = vi.fn();
    h.store.headersCb!({ responseHeaders: { "X-Test": ["1"] } }, callback);
    expect(callback).toHaveBeenCalledWith({
      responseHeaders: expect.objectContaining({
        "X-Test": ["1"],
        "Content-Security-Policy": [expect.stringContaining("default-src 'self'")],
      }),
    });

    expect(h.loadFile).toHaveBeenCalledTimes(1);
    expect(h.loadURL).not.toHaveBeenCalled();
  });

  it("loads the dev server URL and skips the CSP in development", async () => {
    await loadMain({ rendererUrl: "http://localhost:5173" });

    expect(h.loadURL).toHaveBeenCalledWith("http://localhost:5173");
    expect(h.loadFile).not.toHaveBeenCalled();
    expect(h.onHeadersReceived).not.toHaveBeenCalled();
    expect(h.store.headersCb).toBeNull();
  });

  it("wires up the window lifecycle handlers", async () => {
    await loadMain();

    h.windowEvents["ready-to-show"]!();
    expect(h.showSpy).toHaveBeenCalledTimes(1);

    const result = h.store.windowOpenHandler!({ url: "https://example.com" });
    expect(h.openExternal).toHaveBeenCalledWith("https://example.com");
    expect(result).toEqual({ action: "deny" });

    expect(() => h.windowEvents["closed"]!()).not.toThrow();
  });

  it("re-creates the window on activate only when none are open", async () => {
    await loadMain();
    expect(h.BrowserWindow).toHaveBeenCalledTimes(1);

    h.state.allWindows = [];
    h.appOn["activate"]!();
    expect(h.BrowserWindow).toHaveBeenCalledTimes(2);

    h.state.allWindows = [{}];
    h.appOn["activate"]!();
    expect(h.BrowserWindow).toHaveBeenCalledTimes(2);
  });

  it("quits when the last window closes except on macOS", async () => {
    await loadMain();

    setPlatform("linux");
    h.appOn["window-all-closed"]!();
    expect(h.app.quit).toHaveBeenCalledTimes(1);

    setPlatform("darwin");
    h.appOn["window-all-closed"]!();
    expect(h.app.quit).toHaveBeenCalledTimes(1);
  });

  it("logs an error if the app never becomes ready", async () => {
    vi.resetModules();
    for (const key of Object.keys(h.handlers)) delete h.handlers[key];
    h.state.whenReadyRejects = true;
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    delete process.env["ELECTRON_RENDERER_URL"];

    await import("./index");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(consoleError).toHaveBeenCalledWith(expect.any(Error));
    expect(h.handlers[IPC.ping]).toBeUndefined();
    consoleError.mockRestore();
  });
});
