// Pin a scroll container to its bottom while content streams in — but respect
// the user: scrolling up unpins (the stream stops yanking the view), scrolling
// back to the bottom re-pins. Used by seat cards, the mediator panel, and the
// event log.

import { ref, watch, type Ref, type WatchSource } from "vue";

/** Px from the bottom still counted as "at the bottom" (rounding + momentum). */
const SLACK = 32;

export function useStickToBottom(el: Ref<HTMLElement | null>, source: WatchSource<unknown>) {
  const pinned = ref(true);

  function onScroll() {
    const node = el.value;
    if (!node) return;
    pinned.value = node.scrollTop + node.clientHeight >= node.scrollHeight - SLACK;
  }

  // flush: "post" → runs after the DOM has the new content, so scrollHeight is
  // current. Direct assignment (not smooth) keeps up with fast token streams.
  watch(
    source,
    () => {
      if (!pinned.value) return;
      const node = el.value;
      if (node) node.scrollTop = node.scrollHeight;
    },
    { flush: "post" },
  );

  return { pinned, onScroll };
}
