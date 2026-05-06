// Compact human-readable "time ago" formatter. Uses Intl.RelativeTimeFormat
// for locale-aware output.

const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

const STEPS: Array<{ unit: Intl.RelativeTimeFormatUnit; seconds: number }> = [
  { unit: "year", seconds: 31_536_000 },
  { unit: "month", seconds: 2_592_000 },
  { unit: "week", seconds: 604_800 },
  { unit: "day", seconds: 86_400 },
  { unit: "hour", seconds: 3_600 },
  { unit: "minute", seconds: 60 },
];

export function timeAgo(iso: string | undefined | null): string {
  if (!iso) return "";
  const ts = new Date(iso).getTime();
  if (Number.isNaN(ts)) return "";

  const diffSec = (Date.now() - ts) / 1000;
  if (diffSec < 30) return "just now";

  for (const step of STEPS) {
    if (diffSec >= step.seconds) {
      const value = Math.floor(diffSec / step.seconds);
      return rtf.format(-value, step.unit);
    }
  }
  return rtf.format(-Math.floor(diffSec), "second");
}
