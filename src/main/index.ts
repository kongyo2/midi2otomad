import { app, BrowserWindow, ipcMain, session, shell } from "electron";
import { join } from "node:path";
import { IPC } from "../shared/ipc";
import type { BounceRequest } from "../shared/media";
import { createEmptyProject } from "../shared/schemas/project";
import { probeMedia } from "./media/backend";
import { bounce, openAudio, openMidi } from "./dialogs";

let mainWindow: BrowserWindow | null = null;

function registerIpcHandlers(): void {
  ipcMain.handle(IPC.ping, () => "pong");
  ipcMain.handle(IPC.getVersion, () => process.versions.electron);
  ipcMain.handle(IPC.defaultProject, () => createEmptyProject());
  ipcMain.handle(IPC.probeMedia, () => probeMedia());
  ipcMain.handle(IPC.openMidi, () => openMidi(mainWindow));
  ipcMain.handle(IPC.openAudio, () => openAudio(mainWindow));
  ipcMain.handle(IPC.bounce, (_event, request: BounceRequest) => bounce(mainWindow, request));
}

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 1024,
    minHeight: 640,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: "#16161c",
    icon: join(__dirname, "../../resources/icon.png"),
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  mainWindow.on("ready-to-show", () => {
    mainWindow?.show();
  });

  mainWindow.on("closed", () => {
    mainWindow = null;
  });

  mainWindow.webContents.setWindowOpenHandler((details) => {
    void shell.openExternal(details.url);
    return { action: "deny" };
  });

  const devServerUrl = process.env["ELECTRON_RENDERER_URL"];
  if (devServerUrl !== undefined) {
    void mainWindow.loadURL(devServerUrl);
  } else {
    void mainWindow.loadFile(join(__dirname, "../renderer/index.html"));
  }
}

function enforceProductionCsp(): void {
  // dev は Vite の HMR がインラインスクリプトを注入するため CSP を課さず、production のみ厳格化する
  if (process.env["ELECTRON_RENDERER_URL"] !== undefined) {
    return;
  }
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        "Content-Security-Policy": [
          "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; media-src 'self' blob: data: file:",
        ],
      },
    });
  });
}

async function bootstrap(): Promise<void> {
  await app.whenReady();
  enforceProductionCsp();
  registerIpcHandlers();
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
}

bootstrap().catch((error: unknown) => {
  console.error(error);
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
