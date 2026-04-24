import { useEffect, useState } from "react";

import { SPLASH_FADE_DURATION_MS } from "../constants";
import { getSplashDurationMs } from "../utils";

export function useSplashScreen() {
  const [visible, setVisible] = useState(true);
  const [fading, setFading] = useState(false);

  useEffect(() => {
    const duration = getSplashDurationMs();
    const fadeTimer = window.setTimeout(() => setFading(true), duration);
    const hideTimer = window.setTimeout(() => setVisible(false), duration + SPLASH_FADE_DURATION_MS);
    return () => {
      window.clearTimeout(fadeTimer);
      window.clearTimeout(hideTimer);
    };
  }, []);

  return { fading, visible };
}
