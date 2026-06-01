/// <reference types="vite/client" />

import type { BridgeApi } from "../../shared/ipc";

declare global {
  interface Window {
    api: BridgeApi;
  }
}
