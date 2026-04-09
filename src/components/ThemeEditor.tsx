import { BUILT_IN_THEMES } from "@/lib/themes";
import { applyThemeTokens, applyUiScale, useThemeStore } from "@/stores/theme";
import type { SchematicCanvasTokens, Theme, ThemeTokens } from "@/types/theme";
import {
  Check,
  Copy,
  Download,
  Pencil,
  Plus,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

// ─── Font options ────────────────────────────────────────────────────────────

const SANS_FONTS = [
  { label: "Roboto", value: '"Roboto", sans-serif' },
  { label: "Inter", value: '"Inter", system-ui, sans-serif' },
  { label: "Segoe UI", value: '"Segoe UI", system-ui, sans-serif' },
  { label: "Calibri", value: '"Calibri", "Segoe UI", sans-serif' },
  { label: "System UI", value: "system-ui, sans-serif" },
  {
    label: "-apple-system",
    value: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  },
];

const MONO_FONTS = [
  { label: "Roboto Mono", value: '"Roboto Mono", monospace' },
  {
    label: "JetBrains Mono",
    value: '"JetBrains Mono", "Fira Code", monospace',
  },
  { label: "Cascadia Code", value: '"Cascadia Code", "Consolas", monospace' },
  { label: "Fira Code", value: '"Fira Code", monospace' },
  { label: "Source Code Pro", value: '"Source Code Pro", monospace' },
  { label: "Consolas", value: '"Consolas", "Courier New", monospace' },
  { label: "SF Mono", value: '"SFMono-Regular", "Cascadia Code", monospace' },
];

// ─── Token group definitions ─────────────────────────────────────────────────

type ColorKey = Exclude<
  keyof ThemeTokens,
  "fontSans" | "fontMono" | "fontSize" | "canvas"
>;

const TOKEN_GROUPS: { title: string; keys: ColorKey[] }[] = [
  {
    title: "Background",
    keys: [
      "bgPrimary",
      "bgSecondary",
      "bgTertiary",
      "bgSurface",
      "bgHover",
      "bgActive",
    ],
  },
  {
    title: "Border",
    keys: ["border", "borderSubtle", "borderActive"],
  },
  {
    title: "Text",
    keys: ["textPrimary", "textSecondary", "textMuted"],
  },
  {
    title: "Accent",
    keys: ["accent", "accentHover", "accentDim"],
  },
  {
    title: "Status",
    keys: ["success", "warning", "error", "info"],
  },
];

const TOKEN_LABELS: Record<ColorKey, string> = {
  bgPrimary: "Primary Background",
  bgSecondary: "Secondary Background",
  bgTertiary: "Tertiary Background",
  bgSurface: "Surface",
  bgHover: "Hover",
  bgActive: "Active",
  border: "Border",
  borderSubtle: "Subtle Border",
  borderActive: "Active Border",
  textPrimary: "Primary Text",
  textSecondary: "Secondary Text",
  textMuted: "Muted Text",
  accent: "Accent",
  accentHover: "Accent Hover",
  accentDim: "Accent Dim",
  success: "Success",
  warning: "Warning",
  error: "Error",
  info: "Info",
};

// Canvas token keys that are editable hex colors (selectionFill is rgba, handled separately)
type CanvasColorKey = Exclude<
  keyof SchematicCanvasTokens,
  "selectionFill" | "schFont"
>;

const CANVAS_TOKEN_GROUPS: { title: string; keys: CanvasColorKey[] }[] = [
  {
    title: "Canvas Area",
    keys: ["bg", "paper", "paperBorder", "grid", "gridMajor"],
  },
  { title: "Wires & Signals", keys: ["wire", "junction", "bus", "busEntry"] },
  {
    title: "Components",
    keys: ["body", "bodyFill", "pin", "pinName", "pinNum"],
  },
  {
    title: "Annotations",
    keys: ["ref", "val", "labelNet", "labelGlobal", "labelHier"],
  },
  {
    title: "Sheets & Special",
    keys: ["sheet", "sheetText", "noConnect", "power"],
  },
  {
    title: "Selection & Handles",
    keys: ["selection", "handleFill", "handleBorder"],
  },
];

const CANVAS_TOKEN_LABELS: Record<CanvasColorKey, string> = {
  bg: "Background",
  paper: "Paper",
  paperBorder: "Paper Border",
  grid: "Grid",
  gridMajor: "Grid Major",
  wire: "Wire",
  junction: "Junction",
  bus: "Bus",
  busEntry: "Bus Entry",
  body: "Component Body",
  bodyFill: "Component Fill",
  pin: "Pin",
  pinName: "Pin Name",
  pinNum: "Pin Number",
  ref: "Reference",
  val: "Value",
  labelNet: "Net Label",
  labelGlobal: "Global Label",
  labelHier: "Hierarchical Label",
  sheet: "Sheet",
  sheetText: "Sheet Text",
  noConnect: "No Connect",
  power: "Power Symbol",
  selection: "Selection",
  handleFill: "Handle Fill",
  handleBorder: "Handle Border",
};

// ─── Sub-components ───────────────────────────────────────────────────────────

function ColorRow({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled: boolean;
}) {
  const [text, setText] = useState(value);

  useEffect(() => {
    setText(value);
  }, [value]);

  const commit = () => {
    const v = text.trim();
    if (/^#[0-9a-fA-F]{3,8}$/.test(v)) onChange(v);
    else setText(value); // revert invalid
  };

  return (
    <div className="flex items-center gap-2 py-0.5">
      <input
        type="color"
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="w-6 h-6 rounded cursor-pointer border border-border-subtle bg-transparent disabled:opacity-40 disabled:cursor-default"
        style={{ padding: "1px", colorScheme: "dark" }}
      />
      <input
        type="text"
        value={text}
        disabled={disabled}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
        className="w-20 bg-bg-primary border border-border-subtle rounded px-1.5 py-0.5 text-[11px] font-mono outline-none focus:border-accent disabled:opacity-40"
      />
      <span className="flex-1 text-[11px] text-text-muted/70 truncate">
        {label}
      </span>
    </div>
  );
}

function InlineRename({
  value,
  onConfirm,
  onCancel,
}: {
  value: string;
  onConfirm: (v: string) => void;
  onCancel: () => void;
}) {
  const [v, setV] = useState(value);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  return (
    <div className="flex items-center gap-1 flex-1">
      <input
        ref={ref}
        value={v}
        onChange={(e) => setV(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onConfirm(v);
          if (e.key === "Escape") onCancel();
        }}
        className="flex-1 bg-bg-primary border border-accent rounded px-2 py-0.5 text-[11px] outline-none"
      />
      <button
        onClick={() => onConfirm(v)}
        className="p-0.5 text-success hover:text-success/80"
      >
        <Check size={12} />
      </button>
      <button
        onClick={onCancel}
        className="p-0.5 text-text-muted hover:text-text-primary"
      >
        <X size={12} />
      </button>
    </div>
  );
}

// ─── Migrate legacy tokens that predate the canvas field ─────────────────────

function ensureCanvas(t: ThemeTokens): ThemeTokens {
  if (t.canvas) return t;
  return { ...t, canvas: BUILT_IN_THEMES[0].tokens.canvas };
}

// ─── Main component ───────────────────────────────────────────────────────────

export function ThemeEditor() {
  const {
    activeThemeId,
    customThemes,
    uiScale,
    getAllThemes,
    setActiveTheme,
    setUiScale,
    updateCustomTheme,
    renameCustomTheme,
    importTheme,
    exportTheme,
    deleteCustomTheme,
  } = useThemeStore();

  const allThemes = getAllThemes();
  const activeTheme =
    allThemes.find((t) => t.id === activeThemeId) ?? BUILT_IN_THEMES[0];
  const isBuiltIn = BUILT_IN_THEMES.some((t) => t.id === activeThemeId);

  // Local edit buffer — updated as user edits colors
  const [tokens, setTokens] = useState<ThemeTokens>(
    ensureCanvas(activeTheme.tokens),
  );
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Sync buffer when active theme changes
  useEffect(() => {
    const t = getAllThemes().find((th) => th.id === activeThemeId);
    if (t) setTokens(ensureCanvas(t.tokens));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeThemeId]);

  const handleColorChange = (key: ColorKey, val: string) => {
    const updated = { ...tokens, [key]: val };
    setTokens(updated);
    if (!isBuiltIn) updateCustomTheme(activeThemeId, updated);
    else applyThemeTokens(updated); // live preview only (not persisted for built-ins)
  };

  const handleFontChange = (key: "fontSans" | "fontMono", val: string) => {
    const updated = { ...tokens, [key]: val };
    setTokens(updated);
    if (!isBuiltIn) updateCustomTheme(activeThemeId, updated);
    else applyThemeTokens(updated);
  };

  // onChange only updates local display (no DOM reflow = no slider jump).
  // Actual DOM + store write happens on pointerUp (drag end).
  const handleFontSizeLocalChange = (val: number) => {
    setTokens((prev) => ({ ...prev, fontSize: val }));
  };

  const handleCanvasColorChange = (key: CanvasColorKey, val: string) => {
    const updated: ThemeTokens = {
      ...tokens,
      canvas: { ...tokens.canvas, [key]: val },
    };
    setTokens(updated);
    if (!isBuiltIn) updateCustomTheme(activeThemeId, updated);
    else applyThemeTokens(updated);
  };

  const handleFontSizeCommit = (val: number) => {
    const updated = { ...tokens, fontSize: val };
    setTokens(updated);
    if (!isBuiltIn) updateCustomTheme(activeThemeId, updated);
    else applyThemeTokens(updated);
  };

  const handleClone = () => {
    const newTheme: Theme = {
      id: `custom-${Date.now()}`,
      name: `${activeTheme.name} (copy)`,
      author: "User",
      version: "1.0",
      tokens: { ...tokens },
    };
    const err = importTheme(JSON.stringify(newTheme));
    if (!err) setActiveTheme(newTheme.id);
  };

  const handleExport = () => {
    const json = exportTheme(activeThemeId);
    if (!json) return;
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${activeThemeId}.theme.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImportFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result as string;
      const err = importTheme(text);
      if (err) {
        setImportError(err);
      } else {
        setImportError(null);
        // Switch to imported theme
        try {
          const parsed = JSON.parse(text) as Theme;
          setActiveTheme(parsed.id);
        } catch {
          /* already validated */
        }
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  };

  return (
    <div className="space-y-4">
      {/* Theme selector row */}
      <div className="flex items-center gap-2">
        <select
          value={activeThemeId}
          onChange={(e) => setActiveTheme(e.target.value)}
          className="flex-1 bg-bg-primary border border-border-subtle rounded px-2 py-1 text-[11px] outline-none focus:border-accent"
        >
          <optgroup label="Built-in Themes">
            {BUILT_IN_THEMES.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </optgroup>
          {customThemes.length > 0 && (
            <optgroup label="Custom Themes">
              {customThemes.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </optgroup>
          )}
        </select>

        {/* Rename custom theme */}
        {!isBuiltIn && renamingId !== activeThemeId && (
          <button
            onClick={() => setRenamingId(activeThemeId)}
            title="Rename"
            className="p-1 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover"
          >
            <Pencil size={13} />
          </button>
        )}

        {/* Clone */}
        <button
          onClick={handleClone}
          title="Clone theme (editable copy)"
          className="p-1 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover"
        >
          <Copy size={13} />
        </button>

        {/* Export */}
        <button
          onClick={handleExport}
          title="Export theme as JSON"
          className="p-1 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover"
        >
          <Download size={13} />
        </button>

        {/* Import */}
        <button
          onClick={() => fileInputRef.current?.click()}
          title="Import theme JSON"
          className="p-1 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover"
        >
          <Upload size={13} />
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept=".json,.theme.json"
          className="hidden"
          onChange={handleImportFile}
        />

        {/* Delete custom */}
        {!isBuiltIn && (
          <button
            onClick={() => deleteCustomTheme(activeThemeId)}
            title="Delete custom theme"
            className="p-1 rounded text-error/70 hover:text-error hover:bg-bg-hover"
          >
            <Trash2 size={13} />
          </button>
        )}

        {/* New blank theme */}
        <button
          onClick={() => {
            const newTheme: Theme = {
              id: `custom-${Date.now()}`,
              name: "New Theme",
              author: "User",
              version: "1.0",
              tokens: { ...BUILT_IN_THEMES[0].tokens },
            };
            importTheme(JSON.stringify(newTheme));
            setActiveTheme(newTheme.id);
          }}
          title="New blank theme"
          className="p-1 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover"
        >
          <Plus size={13} />
        </button>
      </div>

      {/* Inline rename */}
      {renamingId === activeThemeId && !isBuiltIn && (
        <InlineRename
          value={activeTheme.name}
          onConfirm={(v) => {
            renameCustomTheme(activeThemeId, v);
            setRenamingId(null);
          }}
          onCancel={() => setRenamingId(null)}
        />
      )}

      {/* Import error */}
      {importError && (
        <div className="text-[11px] text-error bg-error/10 border border-error/20 rounded px-2 py-1">
          {importError}
        </div>
      )}

      {/* Info banner for built-in themes */}
      {isBuiltIn && (
        <div className="text-[11px] text-info bg-info/10 border border-info/20 rounded px-2 py-1">
          Clone this theme to edit it <Copy size={10} className="inline" />
        </div>
      )}

      {/* Typography */}
      <div>
        <h4 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-2">
          Typography
        </h4>
        <div className="space-y-2 pl-1">
          <div className="flex items-center gap-2">
            <span className="w-20 text-[11px] text-text-muted/70 shrink-0">
              Sans Font
            </span>
            <select
              value={tokens.fontSans}
              disabled={isBuiltIn}
              onChange={(e) => handleFontChange("fontSans", e.target.value)}
              className="flex-1 bg-bg-primary border border-border-subtle rounded px-2 py-0.5 text-[11px] outline-none focus:border-accent disabled:opacity-50"
            >
              {SANS_FONTS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
              {!SANS_FONTS.some((f) => f.value === tokens.fontSans) && (
                <option value={tokens.fontSans}>(custom)</option>
              )}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-20 text-[11px] text-text-muted/70 shrink-0">
              Mono Font
            </span>
            <select
              value={tokens.fontMono}
              disabled={isBuiltIn}
              onChange={(e) => handleFontChange("fontMono", e.target.value)}
              className="flex-1 bg-bg-primary border border-border-subtle rounded px-2 py-0.5 text-[11px] outline-none focus:border-accent disabled:opacity-50"
            >
              {MONO_FONTS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
              {!MONO_FONTS.some((f) => f.value === tokens.fontMono) && (
                <option value={tokens.fontMono}>(custom)</option>
              )}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-20 text-[11px] text-text-muted/70 shrink-0">
              Font Size
            </span>
            <div className="flex items-center gap-2">
              <input
                type="range"
                min={10}
                max={16}
                step={1}
                value={tokens.fontSize}
                disabled={isBuiltIn}
                onChange={(e) =>
                  handleFontSizeLocalChange(parseInt(e.target.value))
                }
                onPointerUp={(e) =>
                  handleFontSizeCommit(
                    parseInt((e.target as HTMLInputElement).value),
                  )
                }
                className="w-24 disabled:opacity-50"
              />
              <span className="text-[11px] font-mono text-text-secondary w-8">
                {tokens.fontSize}px
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* UI Scale */}
      <div>
        <h4 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-2">
          UI Scale
        </h4>
        <div className="space-y-2 pl-1">
          <div className="flex items-center gap-2">
            <span className="w-20 text-[11px] text-text-muted/70 shrink-0">
              Zoom
            </span>
            <div className="flex items-center gap-2">
              <input
                type="range"
                min={75}
                max={150}
                step={5}
                value={Math.round(uiScale * 100)}
                onChange={(e) => {
                  const pct = parseInt(e.target.value);
                  applyUiScale(pct / 100);
                }}
                onPointerUp={(e) => {
                  const pct = parseInt((e.target as HTMLInputElement).value);
                  setUiScale(pct / 100);
                }}
                className="w-28"
              />
              <span className="text-[11px] font-mono text-text-secondary w-10">
                {Math.round(uiScale * 100)}%
              </span>
            </div>
          </div>
          <div className="flex items-center gap-2 flex-wrap">
            {[75, 87, 100, 112, 125, 150].map((pct) => (
              <button
                key={pct}
                onClick={() => setUiScale(pct / 100)}
                className={`px-2 py-0.5 rounded text-[10px] border transition-colors ${
                  Math.round(uiScale * 100) === pct
                    ? "border-accent text-accent bg-accent/10"
                    : "border-border-subtle text-text-muted hover:border-accent/50 hover:text-text-secondary"
                }`}
              >
                {pct}%
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Color tokens by group */}
      {TOKEN_GROUPS.map((group) => (
        <div key={group.title}>
          <h4 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-1">
            {group.title}
          </h4>
          <div className="pl-1">
            {group.keys.map((key) => (
              <ColorRow
                key={key}
                label={TOKEN_LABELS[key]}
                value={tokens[key] as string}
                disabled={isBuiltIn}
                onChange={(v) => handleColorChange(key, v)}
              />
            ))}
          </div>
        </div>
      ))}

      {/* Schematic Canvas Colors */}
      <div>
        <h4 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-1">
          Schematic Canvas
        </h4>
        <p className="text-[10px] text-text-muted mb-2">
          Controls the drawing area independently from the UI.
        </p>
        {CANVAS_TOKEN_GROUPS.map((group) => (
          <div key={group.title} className="mb-2">
            <h5 className="text-[10px] font-medium text-text-muted tracking-wide mb-1">
              {group.title}
            </h5>
            <div className="pl-1">
              {group.keys.map((key) => (
                <ColorRow
                  key={key}
                  label={CANVAS_TOKEN_LABELS[key]}
                  value={tokens.canvas[key]}
                  disabled={isBuiltIn}
                  onChange={(v) => handleCanvasColorChange(key, v)}
                />
              ))}
            </div>
          </div>
        ))}
        {/* Canvas preview */}
        <div
          className="rounded border text-[10px] mt-2 overflow-hidden"
          style={{
            borderColor: tokens.canvas.paperBorder,
            backgroundColor: tokens.canvas.bg,
          }}
        >
          <div
            className="p-3 space-y-1"
            style={{ backgroundColor: tokens.canvas.paper }}
          >
            <div className="flex items-center gap-3">
              <span style={{ color: tokens.canvas.ref }}>U1</span>
              <span style={{ color: tokens.canvas.val }}>ATmega328P</span>
              <span
                style={{
                  color: tokens.canvas.labelNet,
                  fontFamily: tokens.fontMono,
                }}
              >
                VCC
              </span>
              <span style={{ color: tokens.canvas.power }}>+3.3V</span>
            </div>
            <div className="flex items-center gap-2">
              <div
                className="h-px w-16"
                style={{ backgroundColor: tokens.canvas.wire }}
              />
              <div
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: tokens.canvas.junction }}
              />
              <div
                className="h-px w-8"
                style={{ backgroundColor: tokens.canvas.bus }}
              />
            </div>
            <div className="flex items-center gap-2">
              <span style={{ color: tokens.canvas.pin }}>PIN</span>
              <span style={{ color: tokens.canvas.noConnect }}>X</span>
              <span style={{ color: tokens.canvas.selection }}>SEL</span>
            </div>
          </div>
        </div>
      </div>

      {/* Live preview strip */}
      <div>
        <h4 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider mb-2">
          UI Preview
        </h4>
        <div
          className="rounded border text-[11px] p-3 space-y-1.5"
          style={{
            backgroundColor: tokens.bgSurface,
            borderColor: tokens.border,
            color: tokens.textPrimary,
            fontFamily: tokens.fontSans,
          }}
        >
          <div className="flex items-center gap-2">
            <span style={{ color: tokens.textPrimary }}>Primary text</span>
            <span style={{ color: tokens.textSecondary }}>Secondary</span>
            <span style={{ color: tokens.textMuted }}>Muted</span>
          </div>
          <div className="flex items-center gap-2">
            <span
              className="px-2 py-0.5 rounded text-[10px]"
              style={{
                backgroundColor: tokens.accent + "33",
                color: tokens.accent,
              }}
            >
              Accent
            </span>
            <span
              className="px-2 py-0.5 rounded text-[10px]"
              style={{
                backgroundColor: tokens.success + "33",
                color: tokens.success,
              }}
            >
              Success
            </span>
            <span
              className="px-2 py-0.5 rounded text-[10px]"
              style={{
                backgroundColor: tokens.warning + "33",
                color: tokens.warning,
              }}
            >
              Warning
            </span>
            <span
              className="px-2 py-0.5 rounded text-[10px]"
              style={{
                backgroundColor: tokens.error + "33",
                color: tokens.error,
              }}
            >
              Error
            </span>
          </div>
          <div
            className="font-mono text-[10px] px-2 py-1 rounded"
            style={{
              backgroundColor: tokens.bgPrimary,
              color: tokens.info,
              fontFamily: tokens.fontMono,
            }}
          >
            const foo = "mono preview";
          </div>
        </div>
      </div>
    </div>
  );
}
