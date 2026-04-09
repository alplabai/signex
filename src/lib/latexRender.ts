/**
 * latexRender.ts
 *
 * KaTeX-backed LaTeX renderer for Canvas2D.
 *
 * Usage pattern:
 *   1. Call setLatexRerenderCallback(render) once per renderer mount.
 *   2. Replace ctx.fillText(text, x, y) with drawLatexLine(ctx, text, x, y, fontSize).
 *
 * LaTeX syntax supported in text:
 *   $...$       → inline math
 *   $$...$$     → display (centered, slightly larger)
 *   D_{x}       → implicit subscript (auto-wrapped as $D_{x}$)
 *   R^{2}       → implicit superscript (auto-wrapped)
 *   V_{CC}^{+}  → chained implicit (auto-wrapped)
 */

import katex from "katex";

// ---------------------------------------------------------------------------
// Internal cache
// ---------------------------------------------------------------------------

interface LatexEntry {
  img: HTMLImageElement;
  /** Natural pixel width of the rendered SVG image */
  naturalW: number;
  /** Natural pixel height of the rendered SVG image */
  naturalH: number;
  /** Fraction of image height above the text baseline (0 = top, 1 = bottom) */
  baselineRatio: number;
}

const CACHE = new Map<string, LatexEntry | "loading" | "error">();

let _rerenderCb: (() => void) | null = null;

/** Register the canvas rerender function — called when a new LaTeX image finishes loading. */
export function setLatexRerenderCallback(cb: () => void): void {
  _rerenderCb = cb;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Auto-wrap bare subscript/superscript notation (_{...} or ^{...}) that is
 * NOT already inside $...$ delimiters.
 *
 * Examples:
 *   D_{x}           → $D_{x}$
 *   VCC_{TEST}      → $VCC_{TEST}$
 *   R^{2}           → $R^{2}$
 *   V_{CC}^{+}      → $V_{CC}^{+}$
 *   Some D_{3} text → Some $D_{3}$ text
 */
export function normalizeImplicitLatex(text: string): string {
  // Pattern: optional word-char prefix + one or more chained _{}^{} groups
  const IMPLICIT = /([A-Za-z0-9\u0080-\uffff]*)(?=[_^]\{)([_^]\{[^}]*\})+/g;
  // Wrap prefix and each subscript/superscript content in \text{} so KaTeX
  // uses the schematic font (via CSS override) instead of math italic.
  const wrapImplicit = (t: string) =>
    t.replace(IMPLICIT, (m) => {
      const texified = m
        // Wrap leading identifier in \text{}
        .replace(/^([A-Za-z0-9\u0080-\uffff]+)/, (r) => `\\text{${r}}`)
        // Wrap each sub/superscript content in \text{}
        .replace(
          /([_^])\{([^}]+)\}/g,
          (_, op, content) => `${op}{\\text{${content}}}`,
        );
      return `$${texified}$`;
    });

  if (!text.includes("$")) return wrapImplicit(text);

  // Has explicit $ spans — only normalise the plain-text regions between them
  const re = /(\$\$[\s\S]+?\$\$|\$[^$\n]+?\$)/g;
  let result = "";
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    result += wrapImplicit(text.slice(last, m.index));
    result += m[0]; // keep existing $...$ unchanged
    last = m.index + m[0].length;
  }
  result += wrapImplicit(text.slice(last));
  return result;
}

/** Quick detection: does text contain $...$, $$...$$, or implicit _{}/^{}? */
export function hasLatex(text: string): boolean {
  return /\$\$[\s\S]+?\$\$|\$[^$\n]+?\$|[_^]\{[^}]+\}/.test(text);
}

interface TextSegment {
  kind: "text" | "math" | "display" | "subsup";
  content: string;
  /** For "subsup" segments: identifier before the _{} or ^{} */
  base?: string;
  /** For "subsup" segments: subscript content */
  sub?: string;
  /** For "subsup" segments: superscript content */
  sup?: string;
}

/**
 * Parse a plain-text string into "text" and "subsup" segments without using KaTeX.
 * Handles patterns like VCC_{TEST}, D_{1}^{+}, R^{2} using native Canvas2D rendering.
 * This preserves the schematic font (Iosevka etc.) with correct baseline alignment.
 */
function parseImplicitSubSup(text: string): TextSegment[] {
  if (!text) return [];
  const segs: TextSegment[] = [];
  const IMPLICIT = /([A-Za-z0-9\u0080-\uffff]*)(?=[_^]\{)((?:[_^]\{[^}]*\})+)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = IMPLICIT.exec(text)) !== null) {
    if (m.index > last)
      segs.push({ kind: "text", content: text.slice(last, m.index) });
    const base = m[1];
    let sub: string | undefined;
    let sup: string | undefined;
    const chainRe = /([_^])\{([^}]*)\}/g;
    let cm: RegExpExecArray | null;
    while ((cm = chainRe.exec(m[2])) !== null) {
      if (cm[1] === "_") sub = cm[2];
      else sup = cm[2];
    }
    segs.push({ kind: "subsup", content: m[0], base, sub, sup });
    last = m.index + m[0].length;
  }
  if (last < text.length)
    segs.push({ kind: "text", content: text.slice(last) });
  return segs;
}

/** Split text into plain / inline-math / display-math / subsup segments. */
export function splitLatex(text: string): TextSegment[] {
  const segments: TextSegment[] = [];
  // Split by explicit $$...$$ and $...$ first; parse implicit sub/sup natively in plain parts
  const re = /\$\$([\s\S]+?)\$\$|\$([^$\n]+?)\$/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) {
      segments.push(...parseImplicitSubSup(text.slice(last, m.index)));
    }
    if (m[1] !== undefined) {
      segments.push({ kind: "display", content: m[1] });
    } else {
      segments.push({ kind: "math", content: m[2] });
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) {
    segments.push(...parseImplicitSubSup(text.slice(last)));
  }
  return segments;
}

// ---------------------------------------------------------------------------
// Rendering pipeline
// ---------------------------------------------------------------------------

const RENDER_PX = 160; // Base font size for KaTeX MathML measurement in px — large = high quality

/**
 * Render a KaTeX expression to an HTMLImageElement via MathML + SVG <foreignObject>.
 *
 * Uses KaTeX's MathML output so WebKit's native MathML engine renders all math
 * symbols (∫, Σ, fractions, accents…) with proper math glyphs from the system
 * math font — no dependency on KaTeX_Main or external font files inside the blob URL.
 * \text{} content (mtext elements) inherits the schematic font via CSS.
 */
function buildSVG(
  expr: string,
  displayMode: boolean,
  color: string,
  fontFamily: string,
): { svg: string; baselineRatio: number } {
  // MathML output: symbols rendered by WebKit's native MathML engine (proper ∫, fractions etc.)
  const html = katex.renderToString(expr, {
    output: "mathml",
    displayMode,
    throwOnError: false,
    trust: false,
  });

  // CSS for both the measurement div and the SVG blob:
  // - math elements: system math font (Latin Modern Math / STIX2 / etc.) for proper glyphs
  // - mtext elements (from \text{}): schematic font so labels stay consistent
  const mathCSS = `
    math { font-size: ${RENDER_PX}px; font-family: math, "Latin Modern Math", serif; }
    mtext { font-family: ${fontFamily}, sans-serif; font-style: normal; }
  `;

  // Measure rendered size AND baseline via a hidden DOM element.
  // WebKit lays out MathML in visibility:hidden divs correctly.
  const div = document.createElement("div");
  div.style.cssText = [
    "position:absolute",
    "visibility:hidden",
    "white-space:nowrap",
    `font-size:${RENDER_PX}px`,
    "line-height:1",
    "top:-9999px",
    "left:-9999px",
  ].join(";");
  const cssStyle = document.createElement("style");
  cssStyle.textContent = mathCSS;
  div.appendChild(cssStyle);
  // Probe span sits at the css text baseline (vertical-align:baseline)
  const probe = document.createElement("span");
  probe.style.cssText =
    "display:inline-block;width:0;height:0;overflow:hidden;vertical-align:baseline;";
  div.insertAdjacentHTML("beforeend", html);
  div.appendChild(probe);
  document.body.appendChild(div);
  const rect = div.getBoundingClientRect();
  const probeRect = probe.getBoundingClientRect();
  const w = Math.max(rect.width, 1);
  const h = Math.max(rect.height, 1);
  const baselineRatio =
    h > 0 ? Math.min(1, Math.max(0, (probeRect.top - rect.top) / h)) : 0.78;
  document.body.removeChild(div);

  // Wrap MathML in SVG <foreignObject>. No external font resources needed —
  // WebKit's MathML renderer uses built-in math fonts for all math symbols.
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg"`,
    ` xmlns:xhtml="http://www.w3.org/1999/xhtml"`,
    ` width="${w}" height="${h}">`,
    `<defs><style>${mathCSS}</style></defs>`,
    `<foreignObject x="0" y="0" width="${w}" height="${h}">`,
    `<div xmlns="http://www.w3.org/1999/xhtml"`,
    ` style="font-size:${RENDER_PX}px;white-space:nowrap;color:${color};">`,
    html,
    `</div>`,
    `</foreignObject>`,
    `</svg>`,
  ].join("");

  return { svg, baselineRatio };
}

/**
 * Returns the cached entry, or kicks off async loading and returns null.
 * When the image finishes loading, _rerenderCb() is called.
 * Color and fontFamily are part of the cache key so different styles are cached separately.
 */
function getEntry(
  expr: string,
  displayMode: boolean,
  color: string,
  fontFamily: string,
): LatexEntry | null {
  const key = `${displayMode ? "$$" : "$"}${expr}$|${color}|${fontFamily}`;
  const cached = CACHE.get(key);

  if (cached === "loading" || cached === "error") return null;
  if (cached && typeof cached === "object") return cached;

  // Not in cache — start async load
  CACHE.set(key, "loading");
  try {
    const { svg: svgStr, baselineRatio } = buildSVG(
      expr,
      displayMode,
      color,
      fontFamily,
    );
    const blob = new Blob([svgStr], { type: "image/svg+xml;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => {
      CACHE.set(key, {
        img,
        naturalW: img.naturalWidth || RENDER_PX * 2,
        naturalH: img.naturalHeight || RENDER_PX,
        baselineRatio,
      });
      URL.revokeObjectURL(url);
      _rerenderCb?.();
    };
    img.onerror = () => {
      CACHE.set(key, "error");
      URL.revokeObjectURL(url);
    };
    img.src = url;
  } catch {
    CACHE.set(key, "error");
  }

  return null;
}

// ---------------------------------------------------------------------------
// Public drawing API
// ---------------------------------------------------------------------------

/**
 * Draw a single line of text — which may contain inline/display LaTeX — onto ctx.
 *
 * The context must already have:
 *   - font set to the correct size (e.g. "1.27px Roboto")
 *   - fillStyle set to the desired text colour
 *   - textBaseline, textAlign as needed
 *   - current transform (translate/rotate) applied
 *
 * @param ctx     Canvas2D rendering context
 * @param text    Text that may contain $...$ or $$...$$ tokens
 * @param x       X position in current transform space
 * @param y       Y position (baseline) in current transform space
 * @param fontSize  World-space font size (same unit as x, y)
 * @returns       Total drawn width in world units
 */
/** Extract the font-family portion of a canvas font string like "1px Roboto, sans-serif" */
function extractFontFamily(ctxFont: string): string {
  const m = ctxFont.match(/[\d.]+(?:px|pt|em|rem|%)(?:\/[\d.]+)?\s+(.+)$/);
  return m ? m[1] : "sans-serif";
}

/**
 * Draw a single line of text — which may contain inline/display LaTeX — onto ctx.
 *
 * The context must already have:
 *   - font set to the correct size (e.g. "1.27px Roboto")
 *   - fillStyle set to the desired text colour
 *   - textBaseline, textAlign as needed
 *   - current transform (translate/rotate) applied
 *
 * @param ctx     Canvas2D rendering context
 * @param text    Text that may contain $...$ or $$...$$ tokens
 * @param x       X position in current transform space
 * @param y       Y position (baseline) in current transform space
 * @param fontSize  World-space font size (same unit as x, y)
 * @returns       Total drawn width in world units
 */
export function drawLatexLine(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  fontSize: number,
): number {
  if (!hasLatex(text)) {
    ctx.fillText(text, x, y);
    return ctx.measureText(text).width;
  }

  const segments = splitLatex(text);
  let cx = x;
  const color = ctx.fillStyle as string;
  const fontFamily = extractFontFamily(ctx.font);
  const baseline = ctx.textBaseline;

  for (const seg of segments) {
    if (seg.kind === "text") {
      ctx.fillText(seg.content, cx, y);
      cx += ctx.measureText(seg.content).width;
    } else if (seg.kind === "subsup") {
      // Native sub/superscript — drawn directly with Canvas2D for font consistency.
      // Avoids the SVG-blob font sandboxing issue; uses the schematic font directly.
      const base = seg.base ?? "";
      const sub = seg.sub;
      const sup = seg.sup;
      const subFontSize = fontSize * 0.65;
      const savedFont = ctx.font;
      const savedBaseline = ctx.textBaseline;

      // Draw base text with current canvas settings
      if (base) ctx.fillText(base, cx, y);
      const baseW = base ? ctx.measureText(base).width : 0;

      // Convert y to alphabetic-baseline space for precise sub/sup vertical offsets
      let alphaY = y;
      switch (baseline) {
        case "top":
          alphaY = y + fontSize * 0.8;
          break;
        case "middle":
          alphaY = y + fontSize * 0.15;
          break;
        case "bottom":
          alphaY = y - fontSize * 0.2;
          break;
        // "alphabetic" / default: alphaY = y (no adjustment)
      }

      // Draw sub/sup at 65% size with alphabetic baseline for precise offset control
      ctx.font = savedFont.replace(/[\d.]+px/, subFontSize.toFixed(3) + "px");
      ctx.textBaseline = "alphabetic";
      let subSupW = 0;
      if (sub) {
        ctx.fillText(sub, cx + baseW, alphaY + fontSize * 0.3);
        subSupW = Math.max(subSupW, ctx.measureText(sub).width);
      }
      if (sup) {
        ctx.fillText(sup, cx + baseW, alphaY - fontSize * 0.4);
        subSupW = Math.max(subSupW, ctx.measureText(sup).width);
      }
      ctx.font = savedFont;
      ctx.textBaseline = savedBaseline;
      cx += baseW + subSupW;
    } else {
      const display = seg.kind === "display";
      const entry = getEntry(seg.content, display, color, fontFamily);

      if (entry && entry.naturalH > 0) {
        // Scale so the KaTeX em (1 em ≈ RENDER_PX) maps to fontSize in world units
        // Display mode is slightly larger (1.4×)
        const scale = (display ? fontSize * 1.4 : fontSize) / RENDER_PX;
        const drawW = entry.naturalW * scale;
        const drawH = entry.naturalH * scale;

        // Compute the vertical offset of the KaTeX baseline within the image
        const baselineOffset = drawH * entry.baselineRatio;

        // Position imgTop so the KaTeX baseline aligns with the canvas y position
        // according to the active textBaseline setting.
        let imgTop: number;
        switch (baseline) {
          case "top":
            // canvas y = top of em box → align KaTeX top with y
            imgTop = y;
            break;
          case "middle":
            // canvas y = middle of em box → KaTeX baseline ≈ y + fontSize*0.15
            imgTop = y + fontSize * 0.15 - baselineOffset;
            break;
          case "bottom":
          case "alphabetic":
          default:
            // canvas y = bottom of em / alphabetic baseline → align KaTeX baseline with y
            imgTop = y - baselineOffset;
            break;
        }

        ctx.drawImage(entry.img, cx, imgTop, drawW, drawH);
        cx += drawW;
      } else {
        // Still loading — show placeholder in muted style
        const placeholder =
          seg.kind === "display" ? `$$${seg.content}$$` : `$${seg.content}$`;
        const prevAlpha = ctx.globalAlpha;
        ctx.globalAlpha *= 0.4;
        ctx.fillText(placeholder, cx, y);
        ctx.globalAlpha = prevAlpha;
        cx += ctx.measureText(placeholder).width;
      }
    }
  }

  return cx - x;
}

/**
 * Prefetch all LaTeX expressions found in a text string.
 * Call this when data is loaded to warm the cache before first render.
 */
export function prefetchLatex(text: string, fontFamily = "sans-serif"): void {
  if (!hasLatex(text)) return;
  for (const seg of splitLatex(text)) {
    if (seg.kind === "math" || seg.kind === "display") {
      getEntry(seg.content, seg.kind === "display", "#ffffff", fontFamily);
    }
  }
}
