export type AppPhase = "setup" | "room" | "verdict";
export type ShortcutAction = "palette" | "convene" | "add-seat" | "export" | "help" | "focus-seat" | "escape";

function isEditable(target: EventTarget | null): boolean {
  const node = target instanceof Element ? target : null;
  if (!node) return false;
  return Boolean(node.closest("input, textarea, select, [contenteditable='true'], [contenteditable=''], [contenteditable]:not([contenteditable='false'])"));
}

export function shortcutFor(event: KeyboardEvent, phase: AppPhase): { action: ShortcutAction; seat?: number } | null {
  if (event.isComposing || event.altKey) return null;
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") return { action: "palette" };
  if (event.ctrlKey || event.metaKey) return null;
  if (isEditable(event.target)) return null;
  if (event.shiftKey) return event.key === "?" ? { action: "help" } : null;
  if (event.key === "Escape") return { action: "escape" };
  if (event.key === "?") return { action: "help" };
  if (phase === "setup" && event.key.toLowerCase() === "c") return { action: "convene" };
  if (phase === "setup" && event.key.toLowerCase() === "a") return { action: "add-seat" };
  if (phase === "verdict" && event.key.toLowerCase() === "e") return { action: "export" };
  if (phase === "room" && /^[1-6]$/.test(event.key)) return { action: "focus-seat", seat: Number(event.key) - 1 };
  return null;
}
