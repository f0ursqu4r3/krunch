<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import SetupScreen from "@/screens/SetupScreen.vue";
import RoomScreen from "@/screens/RoomScreen.vue";
import VerdictScreen from "@/screens/VerdictScreen.vue";

const store = useDeliberation();

// Ease the ambient warmth toward the store's target so state changes feel like
// the room slowly warming, not snapping.
const warmthStyle = computed(() => ({ "--warmth": store.warmth.toFixed(3) }));

onMounted(async () => {
  store.init();
  if (import.meta.env.DEV) {
    const kind = new URLSearchParams(location.search).get("preview");
    if (kind) {
      const { seedPreview } = await import("@/lib/preview-seed");
      seedPreview(store, kind);
    }
  }
});
</script>

<template>
  <div class="relative h-full w-full overflow-hidden" :style="warmthStyle">
    <!-- Ambient chamber: candlelight from above that warms with consensus. -->
    <div class="pointer-events-none fixed inset-0 z-0">
      <div class="absolute inset-0" style="background:
        radial-gradient(120% 80% at 50% -10%,
          color-mix(in oklch, var(--brass) calc(var(--warmth) * 26%), transparent) 0%,
          transparent 55%),
        radial-gradient(140% 100% at 50% 120%, var(--bg-deep) 0%, transparent 60%);" />
      <!-- edge vignette -->
      <div class="absolute inset-0" style="box-shadow: inset 0 0 200px 40px var(--bg-deep);" />
    </div>

    <div class="relative z-10 h-full w-full">
      <SetupScreen v-if="store.phase === 'setup'" />
      <RoomScreen v-else-if="store.phase === 'room'" />
      <VerdictScreen v-else />
    </div>
  </div>
</template>
