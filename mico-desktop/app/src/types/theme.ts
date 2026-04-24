export const THEMES = {
  DARK: "dark",
  LIGHT: "light",
} as const;

export type ThemeName = (typeof THEMES)[keyof typeof THEMES];
