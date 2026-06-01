import { app, BrowserWindow, ipcMain, session, shell } from "electron";
import { join } from "node:path";
import { IPC } from "../shared/ipc";
import { parseProject, type Project } from "../shared/schemas/project";
import { probeMedia } from "./media/backend";

function createDefaultProject(): Project {
  return parseProject({
    version: 1,
    name: "Untitled 音MAD",
    bpm: 140,
    ppq: 480,
    samples: [],
    tracks: [],
  });
}

function registerIpcHandlers(): void {
  ipcMain.handle(IPC.ping, () => "pong");
  ipcMain.handle(IPC.getVersion, () => process.versions.electron);
  ipcMain.handle(IPC.defaultProject, () => createDefaultProject());
  ipcMain.handle(IPC.probeMedia, () => probeMedia());
}

function createWindow(): void {
  const mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 600,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: "#1e1e24",
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  mainWindow.on("ready-to-show", () => {
    mainWindow.show();
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
