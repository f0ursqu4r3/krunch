// Smooth the store's ~33ms token flushes into a per-frame, character-level
// reveal. The reveal rate adapts to its backlog (drains in ~CATCHUP_MS) so a
// fast stream never runs away from the animation, and a slow one still types
// letter by letter. When `animate` is false (reduced effects / stream done)
// the text snaps to complete.

import { computed, onBeforeUnmount, ref, watch } from "vue";

/** Target time to drain whatever backlog has accumulated. */
const CATCHUP_MS = 220;

export function useTypewriter(target: () => string, animate: () => boolean) {
  const shown = ref(0);
  let frame = 0;
  let last = 0;

  function stop() {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    last = 0;
  }

  function step(at: number) {
    frame = 0;
    const full = target().length;
    if (!animate()) { shown.value = full; last = 0; return; }
    const dt = last ? Math.min(at - last, 64) : 17;
    last = at;
    const backlog = full - shown.value;
    if (backlog > 0) shown.value += Math.max(1, Math.round((backlog * dt) / CATCHUP_MS));
    if (shown.value < full) frame = requestAnimationFrame(step);
    else last = 0;
  }

  watch(
    [target, animate],
    ([text, on]) => {
      if (!on) { stop(); shown.value = text.length; return; }
      if (text.length < shown.value) shown.value = text.length; // round reset / retry
      if (text.length > shown.value && !frame) frame = requestAnimationFrame(step);
    },
    { immediate: true },
  );

  onBeforeUnmount(stop);

  const text = computed(() => (shown.value >= target().length ? target() : target().slice(0, shown.value)));
  const typing = computed(() => animate() && shown.value < target().length);
  return { text, typing };
}
