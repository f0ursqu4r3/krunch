<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { watchDebounced } from "@vueuse/core";
import { useDeliberation } from "@/stores/deliberation";
import { useSettings } from "@/stores/settings";
import { shortcutFor, type ShortcutAction } from "@/lib/shortcuts";
import SetupScreen from "@/screens/SetupScreen.vue";
import RoomScreen from "@/screens/RoomScreen.vue";
import VerdictScreen from "@/screens/VerdictScreen.vue";
import CockpitStatusBar from "@/components/CockpitStatusBar.vue";
import CommandPalette from "@/components/CommandPalette.vue";
import HistoryDialog from "@/components/HistoryDialog.vue";

const store = useDeliberation();
const settings = useSettings();
const palette = ref(false); const history = ref(false); const booting = ref(true); const effects = ref<"off" | "ambient" | "max">((localStorage.getItem("krunch-effects") as "off" | "ambient" | "max" | null) ?? "ambient"); const autoReduced = ref(false); const reducedMotion = ref(matchMedia("(prefers-reduced-motion: reduce)").matches);
let restoreFocus: HTMLElement | null = null; let frame = 0; let lastFrame = 0; let lastWindow = 0; let motionMedia: MediaQueryList | null = null; const samples: { at: number; duration: number }[] = []; let badWindows = 0; let goodSince = 0;
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
watch(effects, (value) => { void settings.persistEffects(value); store.setReducedEffects(value === "off" || reducedMotion.value || autoReduced.value); });
watch([autoReduced, reducedMotion], () => store.setReducedEffects(effects.value === "off" || reducedMotion.value || autoReduced.value), { immediate: true });
// Persist the working setup (debounced) so the next launch restores it. Only
// while editing — never let a running/finished session's state overwrite it.
watchDebounced(
  () => [store.problem, store.mode, store.maxRounds, store.quorumFraction, store.confidenceFloor, store.seats] as const,
  () => { if (store.phase === "setup") void settings.saveLastSetup(store.snapshotSetup(true)); },
  { debounce: 500, deep: true },
);
function openPalette() { restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null; palette.value = true; }
async function closePalette() { palette.value = false; await nextTick(); restoreFocus?.focus(); restoreFocus = null; }
async function exportDump() { try { const text = await store.exportMarkdown(); await navigator.clipboard.writeText(text); } catch { /* Tauri/API unavailable in preview */ } }
function act(action: ShortcutAction, seat?: number) { if (action === "palette") return openPalette(); if (action === "escape") return void (palette.value ? closePalette() : booting.value = false); if (action === "convene") return void store.start(); if (action === "add-seat") return void store.addPanelist(); if (action === "abort") return void store.abandon(); if (action === "new-session") return void store.newSession(); if (action === "export") return void exportDump(); if (action === "help") return openPalette(); if (action === "history") return void (history.value = true); if (action === "focus-seat" && seat !== undefined) document.querySelector<HTMLElement>(`[data-seat-index='${seat}']`)?.focus(); }
function keydown(event: KeyboardEvent) { const result = shortcutFor(event, store.phase); if (!result) return; event.preventDefault(); act(result.action, result.seat); }
function sample(at: number) { if (lastFrame) samples.push({ at, duration: at - lastFrame }); lastFrame = at; while (samples[0]?.at < at - 1000) samples.shift(); if (effects.value === "ambient" && at - lastWindow >= 1000 && samples.length) { lastWindow = at; const mean = samples.reduce((sum, item) => sum + item.duration, 0) / samples.length; if (mean > 24) { badWindows += 1; goodSince = 0; if (badWindows >= 2) autoReduced.value = true; } else if (mean < 18) { badWindows = 0; goodSince ||= at; if (at - goodSince >= 3000) autoReduced.value = false; } else { badWindows = 0; goodSince = 0; } } frame = requestAnimationFrame(sample); }
const syncMotion = () => { if (motionMedia) reducedMotion.value = motionMedia.matches; };
// The signature moment: the room warms and brightens with the panel's mean
// confidence and cluster fraction. Hue rides a heat scale — amber while it
// deliberates, gold at consensus, cooling to ember red at deadlock.
const glow = computed(() => {
  const c = store.convergence;
  if (!c) return { hue: 74, intensity: store.running ? 0.18 : 0.1 };
  if (c.effectiveRuling === "DEADLOCK") return { hue: 33, intensity: 0.55 };
  if (c.effectiveRuling === "CONSENSUS") return { hue: 86, intensity: 0.92 };
  const lit = Math.max(c.meanConfidence ?? 0, c.clusterFraction ?? 0);
  return { hue: 74 + lit * 8, intensity: 0.16 + lit * 0.5 };
});
const glowStyle = computed(() => ({ "--glow-hue": String(glow.value.hue), "--glow-intensity": String(glow.value.intensity) }));
onMounted(async () => {
  motionMedia = matchMedia("(prefers-reduced-motion: reduce)"); motionMedia.addEventListener("change", syncMotion); store.init();
  const restored = await settings.loadLastSetup();
  if (restored) store.hydrateSetup(restored, { problem: true });
  void settings.loadPresets();
  effects.value = await settings.reconcileEffects(effects.value);
  document.addEventListener("keydown", keydown); frame = requestAnimationFrame(sample); window.setTimeout(() => booting.value = false, reduced() ? 0 : 520); if (import.meta.env.DEV) { const kind = new URLSearchParams(location.search).get("preview"); if (kind) { const { seedPreview } = await import("@/lib/preview-seed"); seedPreview(store, kind); } }
});
onBeforeUnmount(() => { document.removeEventListener("keydown", keydown); cancelAnimationFrame(frame); motionMedia?.removeEventListener("change", syncMotion); });
</script>

<template>
  <div class="chamber relative flex h-full flex-col overflow-hidden" :class="{ 'effects-reduced': autoReduced }" :style="{ '--effects-intensity': effects === 'off' || autoReduced ? 0 : effects === 'max' ? 1 : .55 }">
    <div class="chamber-glow" :style="glowStyle" />
    <CockpitStatusBar :effects="effects" @update:effects="effects = $event" @palette="openPalette" />
    <div class="relative z-10 flex min-h-0 flex-1 flex-col"><SetupScreen v-if="store.phase === 'setup'" class="boot" @history="history = true" /><RoomScreen v-else class="boot" /><VerdictScreen v-if="store.phase === 'verdict'" /></div>
    <CommandPalette :open="palette" :phase="store.phase" @update:open="$event ? openPalette() : closePalette()" @action="act" />
    <HistoryDialog v-model:open="history" />
    <Transition name="fade">
      <button v-if="booting" class="no-press absolute inset-0 z-50 grid place-items-center bg-bg" @click="booting = false">
        <div class="boot text-center">
          <p class="font-display text-6xl text-brass">Krunch</p>
          <p class="mt-4 font-mono text-[10px] uppercase tracking-[0.34em] text-fg-faint">the deliberation chamber</p>
        </div>
      </button>
    </Transition>
  </div>
</template>
