import { useEffect, useState } from "react";

import { DEFAULT_THEME, INSPECTOR_DEFAULT_WIDTH, INSPECTOR_MAX_WIDTH, INSPECTOR_MIN_WIDTH, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH } from "../constants";
import { clamp } from "../utils";
import { THEMES, type ThemeName } from "../../types";

export function useLayoutPreferences(repoCount: number) {
  const [theme, setTheme] = useState<ThemeName>(DEFAULT_THEME);
  const [repoNavExpanded, setRepoNavExpanded] = useState(true);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT_WIDTH);
  const [inspectorWidth, setInspectorWidth] = useState(INSPECTOR_DEFAULT_WIDTH);

  useEffect(() => {
    try {
      const storedTheme = window.localStorage.getItem("mico.theme");
      if (storedTheme === THEMES.LIGHT || storedTheme === THEMES.DARK) {
        setTheme(storedTheme);
      }
    } catch {
      // Theme persistence is best-effort.
    }
  }, []);

  useEffect(() => {
    document.body.dataset.theme = theme;
    try {
      window.localStorage.setItem("mico.theme", theme);
    } catch {
      // Theme persistence is best-effort.
    }
    return () => {
      delete document.body.dataset.theme;
    };
  }, [theme]);

  useEffect(() => {
    try {
      const storedCollapsed = window.localStorage.getItem("mico.sidebarCollapsed");
      const storedSidebarWidth = window.localStorage.getItem("mico.sidebarWidth");
      const storedInspectorWidth = window.localStorage.getItem("mico.inspectorWidth");
      if (storedCollapsed !== null) {
        setSidebarCollapsed(storedCollapsed === "true");
      }
      if (storedSidebarWidth) {
        const parsed = Number.parseInt(storedSidebarWidth, 10);
        if (Number.isFinite(parsed)) {
          setSidebarWidth(clamp(parsed, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH));
        }
      }
      if (storedInspectorWidth) {
        const parsed = Number.parseInt(storedInspectorWidth, 10);
        if (Number.isFinite(parsed)) {
          setInspectorWidth(clamp(parsed, INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH));
        }
      }
    } catch {
      // Layout persistence is best-effort.
    }
  }, []);

  useEffect(() => {
    try {
      const storedExpanded = window.localStorage.getItem("mico.repoNavExpanded");
      if (storedExpanded !== null) {
        setRepoNavExpanded(storedExpanded === "true");
      } else {
        setRepoNavExpanded(repoCount === 0);
      }
    } catch {
      setRepoNavExpanded(repoCount === 0);
    }
  }, [repoCount]);

  useEffect(() => {
    try {
      window.localStorage.setItem("mico.repoNavExpanded", String(repoNavExpanded));
    } catch {
      // Renderer preference persistence is best-effort only.
    }
  }, [repoNavExpanded]);

  useEffect(() => {
    try {
      window.localStorage.setItem("mico.sidebarCollapsed", String(sidebarCollapsed));
      window.localStorage.setItem("mico.sidebarWidth", String(sidebarWidth));
      window.localStorage.setItem("mico.inspectorWidth", String(inspectorWidth));
    } catch {
      // Layout persistence is best-effort only.
    }
  }, [inspectorWidth, sidebarCollapsed, sidebarWidth]);

  return {
    inspectorWidth,
    repoNavExpanded,
    setInspectorWidth,
    setRepoNavExpanded,
    setSidebarCollapsed,
    setSidebarWidth,
    setTheme,
    sidebarCollapsed,
    sidebarWidth,
    theme,
  };
}
