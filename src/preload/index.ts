import { contextBridge, ipcRenderer } from "electron";
import { IPC, type BridgeApi } from "../shared/ipc";

const api: BridgeApi = {
  ping: () => ipcRenderer.invoke(IPC.ping),
  getVersion: () => ipcRenderer.invoke(IPC.getVersion),
  defaultProject: () => ipcRenderer.invoke(IPC.defaultProject),
  probeMedia: () => ipcRenderer.invoke(IPC.probeMedia),
  openMidi: () => ipcRenderer.invoke(IPC.openMidi),
  openAudio: () => ipcRenderer.invoke(IPC.openAudio),
  bounce: (request) => ipcRenderer.invoke(IPC.bounce, request),
};

try {
  contextBridge.exposeInMainWorld("api", api);
} catch (error) {
  console.error(error);
}
