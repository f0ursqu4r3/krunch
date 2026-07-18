<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Cpu, Gauge, Terminal } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import ConvergenceStrip from "@/components/ConvergenceStrip.vue";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

const props = defineProps<{ effects: "off" | "ambient" | "max" }>();
const emit = defineEmits<{ "update:effects": [value: "off" | "ambient" | "max"]; palette: [] }>();
const store = useDeliberation(); const now = ref(Date.now()); let interval = 0;
onMounted(() => { interval = window.setInterval(() => now.value = Date.now(), 1000); }); onBeforeUnmount(() => clearInterval(interval));
const elapsed = computed(() => store.running ? Math.max(0, Math.floor((now.value - (store.acceptedUsage[0]?.receivedAt ?? now.value)) / 1000)) : 0);
const clock = computed(() => `${String(Math.floor(elapsed.value / 60)).padStart(2, "0")}:${String(elapsed.value % 60).padStart(2, "0")}`);
const round = computed(() => Math.min(store.maxRounds, Math.max(0, store.currentRound + 1)));
</script>

<template>
  <header class="terminal-panel relative z-20 flex h-14 shrink-0 items-center gap-4 border-x-0 border-t-0 px-4">
    <div class="flex items-center gap-2 text-cyan"><Terminal class="size-4" /><span class="font-display text-sm">KRUNCH//MC</span></div>
    <div class="hidden border-l border-line pl-4 font-mono text-[10px] text-fg-muted xl:block">SID {{ store.sessionId?.slice(0, 8) ?? "OFFLINE" }} · {{ clock }}</div>
    <div class="font-mono text-[10px] text-fg-muted">{{ store.finalState === 'finalizing' ? 'SYNTHESIZING…' : `R${String(round).padStart(2, '0')}/${String(store.maxRounds).padStart(2, '0')}` }}</div>
    <ConvergenceStrip class="hidden lg:block" />
    <div class="ml-auto flex items-center gap-3 font-mono text-[10px]">
      <span class="hidden text-fg-muted md:inline"><Cpu class="mr-1 inline size-3 text-consensus" />{{ store.usageSummary.total.toLocaleString() }} tok <b :class="store.usageSummary.exact ? 'text-consensus' : 'text-brass'">{{ store.usageSummary.exact ? 'exact' : 'partial' }}</b></span>
      <span class="hidden text-fg-faint lg:inline"><Gauge class="mr-1 inline size-3" />{{ store.approximateOutputRate ? `~${store.approximateOutputRate.toFixed(1)} tok/s` : '~— tok/s' }}</span>
      <span class="hidden text-fg-muted xl:inline">est. {{ store.estimatedCost === null ? '—' : `$${store.estimatedCost.toFixed(4)}` }}</span>
      <ToggleGroup :model-value="props.effects" type="single" size="sm" variant="outline" class="hidden border border-line sm:flex" @update:model-value="emit('update:effects', ($event || 'ambient') as 'off' | 'ambient' | 'max')"><ToggleGroupItem value="off">Off</ToggleGroupItem><ToggleGroupItem value="ambient">Ambient</ToggleGroupItem><ToggleGroupItem value="max">Max</ToggleGroupItem></ToggleGroup>
      <button class="border border-line px-2 py-1 text-fg-faint hover:border-cyan hover:text-cyan" @click="emit('palette')">⌘K</button>
    </div>
  </header>
</template>
