/** Colors used by the Canvas2D schematic renderer. */
export interface SchematicCanvasTokens {
  bg: string;
  paper: string;
  paperBorder: string;
  grid: string;
  gridMajor: string;
  wire: string;
  junction: string;
  body: string;
  bodyFill: string;
  pin: string;
  pinName: string;
  pinNum: string;
  ref: string;
  val: string;
  labelNet: string;
  labelGlobal: string;
  labelHier: string;
  sheet: string;
  sheetText: string;
  noConnect: string;
  power: string;
  selection: string;
  selectionFill: string;
  bus: string;
  busEntry: string;
  handleFill: string;
  handleBorder: string;
}

/** All design tokens that make up a Signex theme. */
export interface ThemeTokens {
  // Backgrounds
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  bgSurface: string;
  bgHover: string;
  bgActive: string;
  // Borders
  border: string;
  borderSubtle: string;
  borderActive: string;
  // Text
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  // Accent
  accent: string;
  accentHover: string;
  accentDim: string;
  // Semantic
  success: string;
  warning: string;
  error: string;
  info: string;
  // Typography
  fontSans: string;
  fontMono: string;
  /** Base font size in px */
  fontSize: number;
  /** Schematic canvas (Canvas2D) color tokens */
  canvas: SchematicCanvasTokens;
}

/** A complete named theme. */
export interface Theme {
  id: string;
  name: string;
  author?: string;
  version?: string;
  tokens: ThemeTokens;
}
