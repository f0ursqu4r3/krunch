// Reflow-free text measurement via Pretext (PLAN §9/§11). Measuring streamed
// transcript text with math instead of DOM queries keeps many simultaneously
// updating seat cards from thrashing browser layout.

import { prepare, layout } from "@chenglou/pretext";

const prepared = new Map<string, unknown>();

async function preparedFor(sample: string, font: string): Promise<unknown> {
  let p = prepared.get(font);
  if (!p) {
    p = await prepare(sample || "Ag", font);
    prepared.set(font, p);
  }
  return p;
}

/** Measure rendered height + line count of `text` at `width` px without the DOM. */
export async function measure(
  text: string,
  width: number,
  font = "14px ui-sans-serif, system-ui, sans-serif",
): Promise<{ height: number; lineCount: number }> {
  try {
    const p = await preparedFor(text, font);
    // Pretext's layout is pure arithmetic; safe to call on every stream tick.
    const res = layout(p as never, Math.max(1, Math.floor(width))) as {
      height: number;
      lineCount: number;
    };
    return { height: res.height ?? 0, lineCount: res.lineCount ?? 0 };
  } catch {
    // Fallback estimate if fonts aren't ready yet.
    const lineCount = Math.max(1, Math.ceil(text.length / Math.max(1, width / 8)));
    return { height: lineCount * 20, lineCount };
  }
}
