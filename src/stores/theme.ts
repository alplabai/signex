import { BUILT_IN_THEMES } from "@/lib/themes";
import type { Theme, ThemeTokens } from "@/types/theme";
import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * Apply UI theme tokens as INLINE styles on <html>.
 * document.documentElement.style.setProperty() sets the highest-priority
 * CSS custom properties — they override @layer theme and @theme rules from
 * Tailwind v4, AND cascade to every descendant including React portals.
 */
export function applyThemeTokens(tokens: ThemeTokens) {
  const r = document.documentElement;
  const s = (v: string, val: string) => r.style.setProperty(v, val);
  s("--color-bg-primary", tokens.bgPrimary);
  s("--color-bg-secondary", tokens.bgSecondary);
  s("--color-bg-tertiary", tokens.bgTertiary);
  s("--color-bg-surface", tokens.bgSurface);
  s("--color-bg-hover", tokens.bgHover);
  s("--color-bg-active", tokens.bgActive);
  s("--color-border", tokens.border);
  s("--color-border-subtle", tokens.borderSubtle);
  s("--color-border-active", tokens.borderActive);
  s("--color-text-primary", tokens.textPrimary);
  s("--color-text-secondary", tokens.textSecondary);
  s("--color-text-muted", tokens.textMuted);
  s("--color-accent", tokens.accent);
  s("--color-accent-hover", tokens.accentHover);
  s("--color-accent-dim", tokens.accentDim);
  s("--color-success", tokens.success);
  s("--color-warning", tokens.warning);
  s("--color-error", tokens.error);
  s("--color-info", tokens.info);
  s("--font-sans", tokens.fontSans);
  s("--font-mono", tokens.fontMono);
  r.style.fontSize = `${tokens.fontSize}px`;
  r.style.fontFamily = tokens.fontSans;
}

/** Apply UI zoom scale (e.g. 1.0 = 100%, 1.25 = 125%) */
export function applyUiScale(scale: number) {
  document.documentElement.style.zoom = String(scale);
}

interface ThemeState {
  activeThemeId: string;
  customThemes: Theme[];
  /** UI zoom scale factor, e.g. 1.0 = 100%, 1.25 = 125% */
  uiScale: number;
  /** Per-user schematic canvas font override, applied on top of the active theme. */
  schFontOverride: string | null;

  /** Returns built-ins + user custom themes. */
  getAllThemes: () => Theme[];
  getActiveTheme: () => Theme;

  setActiveTheme: (id: string) => void;
  setUiScale: (scale: number) => void;
  /** Set the schematic canvas font family (overrides active theme's schFont). */
  setSchFont: (font: string) => void;
  /** Update tokens for a custom theme; no-op for built-ins. */
  updateCustomTheme: (id: string, tokens: ThemeTokens) => void;
  /** Rename a custom theme. */
  renameCustomTheme: (id: string, name: string) => void;
  /** Import a theme from JSON string. Returns an error string on failure. */
  importTheme: (json: string) => string | null;
  /** Export the theme with the given id as a JSON string. */
  exportTheme: (id: string) => string | null;
  /** Delete a custom theme by id. Falls back to built-in if it was active. */
  deleteCustomTheme: (id: string) => void;
  /** Re-apply the active theme (call once on app mount). */
  applyActiveTheme: () => void;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      activeThemeId: "catppuccin-mocha",
      customThemes: [],
      uiScale: 1.0,
      schFontOverride: null,

      getAllThemes: () => [...BUILT_IN_THEMES, ...get().customThemes],

      getActiveTheme: () => {
        const all = get().getAllThemes();
        return (
          all.find((t) => t.id === get().activeThemeId) ?? BUILT_IN_THEMES[0]
        );
      },

      setActiveTheme: (id) => {
        set({ activeThemeId: id });
        const theme = get()
          .getAllThemes()
          .find((t) => t.id === id);
        if (theme) applyThemeTokens(theme.tokens);
      },

      setUiScale: (scale) => {
        set({ uiScale: scale });
        applyUiScale(scale);
      },

      setSchFont: (font) => {
        set({ schFontOverride: font || null });
      },

      updateCustomTheme: (id, tokens) => {
        // Reject updates to built-in themes
        if (BUILT_IN_THEMES.some((t) => t.id === id)) return;
        set((s) => ({
          customThemes: s.customThemes.map((t) =>
            t.id === id ? { ...t, tokens } : t,
          ),
        }));
        if (get().activeThemeId === id) applyThemeTokens(tokens);
      },

      renameCustomTheme: (id, name) => {
        if (BUILT_IN_THEMES.some((t) => t.id === id)) return;
        set((s) => ({
          customThemes: s.customThemes.map((t) =>
            t.id === id ? { ...t, name } : t,
          ),
        }));
      },

      importTheme: (json) => {
        let theme: Theme;
        try {
          theme = JSON.parse(json) as Theme;
        } catch {
          return "Invalid JSON.";
        }
        if (!theme.id || !theme.name || !theme.tokens) {
          return "Invalid theme format (id, name, tokens required).";
        }
        const requiredKeys: (keyof ThemeTokens)[] = [
          "bgPrimary",
          "bgSecondary",
          "bgTertiary",
          "bgSurface",
          "bgHover",
          "bgActive",
          "border",
          "borderSubtle",
          "borderActive",
          "textPrimary",
          "textSecondary",
          "textMuted",
          "accent",
          "accentHover",
          "accentDim",
          "success",
          "warning",
          "error",
          "info",
          "fontSans",
          "fontMono",
          "fontSize",
        ];
        for (const k of requiredKeys) {
          if (theme.tokens[k] === undefined) return `Missing token: ${k}`;
        }
        // canvas tokens are optional — fall back to Catppuccin Mocha defaults
        if (!theme.tokens.canvas) {
          theme.tokens.canvas = { ...BUILT_IN_THEMES[0].tokens.canvas };
        }
        if (BUILT_IN_THEMES.some((t) => t.id === theme.id)) {
          return "Cannot overwrite a built-in theme.";
        }
        set((s) => ({
          customThemes: [
            ...s.customThemes.filter((t) => t.id !== theme.id),
            theme,
          ],
        }));
        return null; // success
      },

      exportTheme: (id) => {
        const theme = get()
          .getAllThemes()
          .find((t) => t.id === id);
        if (!theme) return null;
        return JSON.stringify(theme, null, 2);
      },

      deleteCustomTheme: (id) => {
        if (BUILT_IN_THEMES.some((t) => t.id === id)) return;
        set((s) => ({
          customThemes: s.customThemes.filter((t) => t.id !== id),
        }));
        if (get().activeThemeId === id) {
          get().setActiveTheme("catppuccin-mocha");
        }
      },

      applyActiveTheme: () => {
        const state = get();
        applyThemeTokens(state.getActiveTheme().tokens);
        applyUiScale(state.uiScale);
      },
    }),
    {
      name: "signex-theme",
      version: 1,
      onRehydrateStorage: () => (state) => {
        if (state) {
          applyThemeTokens(state.getActiveTheme().tokens);
          applyUiScale(state.uiScale);
        }
      },
    },
  ),
);
