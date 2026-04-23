import { DEFAULT_SPLASH_DURATION_MS } from "./constants";

export function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export function getSplashDurationMs() {
  try {
    const stored = window.localStorage.getItem("mico.splashDurationMs");
    if (!stored) {
      return DEFAULT_SPLASH_DURATION_MS;
    }
    const parsed = Number.parseInt(stored, 10);
    if (Number.isFinite(parsed) && parsed >= 0) {
      return parsed;
    }
  } catch {
    // Splash duration persistence is best-effort.
  }
  return DEFAULT_SPLASH_DURATION_MS;
}

export function getErrorMessage(caught: unknown, fallback: string) {
  return caught instanceof Error ? caught.message : fallback;
}
