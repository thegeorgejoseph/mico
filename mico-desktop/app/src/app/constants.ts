import { AGENT_KINDS, THEMES, type MicoState } from "../types";

export const emptyState: MicoState = {
  version: 1,
  repos: [],
  worktrees: [],
  sessions: [],
  notifications: [],
  selection: { repoId: "", worktreeId: "", sessionId: "", mode: "effort" },
  logs: [],
  migrations: [],
};

export const DEFAULT_AGENT = AGENT_KINDS.TERMINAL;
export const DEFAULT_COMMAND_PROVIDER = AGENT_KINDS.CODEX;
export const DEFAULT_THEME = THEMES.DARK;

export const MICO_ASCII_WORDMARK = String.raw`
███╗   ███╗██╗ ██████╗ ██████╗
████╗ ████║██║██╔════╝██╔═══██╗
██╔████╔██║██║██║     ██║   ██║
██║╚██╔╝██║██║██║     ██║   ██║
██║ ╚═╝ ██║██║╚██████╗╚██████╔╝
╚═╝     ╚═╝╚═╝ ╚═════╝ ╚═════╝
`;

export const DEFAULT_SPLASH_DURATION_MS = 5000;
export const SPLASH_FADE_DURATION_MS = 320;
export const SIDEBAR_DEFAULT_WIDTH = 272;
export const SIDEBAR_COLLAPSED_WIDTH = 84;
export const SIDEBAR_MIN_WIDTH = 224;
export const SIDEBAR_MAX_WIDTH = 360;
export const INSPECTOR_DEFAULT_WIDTH = 390;
export const INSPECTOR_MIN_WIDTH = 300;
export const INSPECTOR_MAX_WIDTH = 560;
