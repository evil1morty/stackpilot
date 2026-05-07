// Tiny ANSI escape-code parser for terminal output (git-style, npm-style,
// scoop output, etc). Handles the common CSI codes:
//   ESC [ <code> m   → set foreground / background / style
// We map to a small set of CSS classes the log viewer renders inline.
//
// Codes recognised:
//   0      reset
//   1      bold
//   2      dim
//   3      italic
//   4      underline
//   30-37  fg standard
//   90-97  fg bright
//   40-47  bg standard
//   100-107 bg bright
//
// Codes we don't recognise are silently dropped; their text is preserved.

const ESC = "\x1b";

export type AnsiStyle = {
  fg?: string; // CSS color
  bg?: string;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underline?: boolean;
};

export type AnsiFragment = {
  text: string;
  style: AnsiStyle;
};

const FG_STANDARD: Record<number, string> = {
  30: "var(--ansi-black, #4a505c)",
  31: "var(--ansi-red, #f87171)",
  32: "var(--ansi-green, #34d399)",
  33: "var(--ansi-yellow, #fbbf24)",
  34: "var(--ansi-blue, #60a5fa)",
  35: "var(--ansi-magenta, #c084fc)",
  36: "var(--ansi-cyan, #22d3ee)",
  37: "var(--ansi-white, #d1d5db)",
};

const FG_BRIGHT: Record<number, string> = {
  90: "var(--ansi-bright-black, #6b7280)",
  91: "var(--ansi-bright-red, #fca5a5)",
  92: "var(--ansi-bright-green, #6ee7b7)",
  93: "var(--ansi-bright-yellow, #fde68a)",
  94: "var(--ansi-bright-blue, #93c5fd)",
  95: "var(--ansi-bright-magenta, #d8b4fe)",
  96: "var(--ansi-bright-cyan, #67e8f9)",
  97: "var(--ansi-bright-white, #f3f4f6)",
};

const BG_STANDARD: Record<number, string> = {
  40: "var(--ansi-black, #4a505c)",
  41: "var(--ansi-red, #f87171)",
  42: "var(--ansi-green, #34d399)",
  43: "var(--ansi-yellow, #fbbf24)",
  44: "var(--ansi-blue, #60a5fa)",
  45: "var(--ansi-magenta, #c084fc)",
  46: "var(--ansi-cyan, #22d3ee)",
  47: "var(--ansi-white, #d1d5db)",
};

const BG_BRIGHT: Record<number, string> = {
  100: "var(--ansi-bright-black, #6b7280)",
  101: "var(--ansi-bright-red, #fca5a5)",
  102: "var(--ansi-bright-green, #6ee7b7)",
  103: "var(--ansi-bright-yellow, #fde68a)",
  104: "var(--ansi-bright-blue, #93c5fd)",
  105: "var(--ansi-bright-magenta, #d8b4fe)",
  106: "var(--ansi-bright-cyan, #67e8f9)",
  107: "var(--ansi-bright-white, #f3f4f6)",
};

function applyCode(style: AnsiStyle, code: number): AnsiStyle {
  if (code === 0) return {};
  if (code === 1) return { ...style, bold: true };
  if (code === 2) return { ...style, dim: true };
  if (code === 3) return { ...style, italic: true };
  if (code === 4) return { ...style, underline: true };
  if (code === 22) return { ...style, bold: false, dim: false };
  if (code === 23) return { ...style, italic: false };
  if (code === 24) return { ...style, underline: false };
  if (code === 39) return { ...style, fg: undefined };
  if (code === 49) return { ...style, bg: undefined };
  if (FG_STANDARD[code]) return { ...style, fg: FG_STANDARD[code] };
  if (FG_BRIGHT[code]) return { ...style, fg: FG_BRIGHT[code] };
  if (BG_STANDARD[code]) return { ...style, bg: BG_STANDARD[code] };
  if (BG_BRIGHT[code]) return { ...style, bg: BG_BRIGHT[code] };
  return style;
}

/**
 * Parse a single line of ANSI-coded text into styled fragments.
 * Empty input yields a single fragment with empty text + no style.
 */
export function parseAnsiLine(line: string): AnsiFragment[] {
  if (!line) return [{ text: "", style: {} }];
  if (!line.includes(ESC)) return [{ text: line, style: {} }];

  const out: AnsiFragment[] = [];
  let style: AnsiStyle = {};
  let i = 0;
  let buf = "";

  const flush = () => {
    if (buf.length === 0) return;
    out.push({ text: buf, style });
    buf = "";
  };

  while (i < line.length) {
    if (line[i] === ESC && line[i + 1] === "[") {
      // Find the terminator (ASCII 64-126).
      let j = i + 2;
      while (j < line.length) {
        const c = line.charCodeAt(j);
        if (c >= 0x40 && c <= 0x7e) break;
        j++;
      }
      const finalChar = line[j];
      if (finalChar === "m") {
        flush();
        const params = line.slice(i + 2, j);
        const codes = params === "" ? [0] : params.split(";").map((s) => Number(s) || 0);
        for (const c of codes) style = applyCode(style, c);
      }
      // Whether we recognised it or not, skip past the terminator.
      i = j + 1;
    } else {
      buf += line[i];
      i++;
    }
  }
  flush();
  if (out.length === 0) out.push({ text: "", style: {} });
  return out;
}

/**
 * Convert an AnsiStyle into an inline `style="..."` string. Returns "" when
 * no styling applies — caller can skip wrapping in a span.
 */
export function styleToCss(s: AnsiStyle): string {
  const parts: string[] = [];
  if (s.fg) parts.push(`color:${s.fg}`);
  if (s.bg) parts.push(`background:${s.bg}`);
  if (s.bold) parts.push("font-weight:600");
  if (s.dim) parts.push("opacity:0.6");
  if (s.italic) parts.push("font-style:italic");
  if (s.underline) parts.push("text-decoration:underline");
  return parts.join(";");
}

/** Strip all ANSI codes — useful for plain-text export / search. */
export function stripAnsi(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/\x1b\[[\d;]*[A-Za-z]/g, "");
}
