import type { MicoApi } from "../types";

declare global {
  interface Window {
    mico: MicoApi;
  }
}

export {};
