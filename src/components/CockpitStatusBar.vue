<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Cpu, Gauge, Gavel } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

const props = defineProps<{ effects: "off" | "ambient" | "max" }>();
const emit = defineEmits<{ "update:effects": [value: "off" | "ambient" | "max"]; palette: [] }>();
const store = useDeliberation(); const now = ref(Date.now()); let interval = 0;
// Two-step abort: first click arms, second confirms; auto-disarm after 3 s so a
// stray click can never single-handedly kill a paid run.
const abortArmed = ref(false); let armTimer = 0;
function abort() {
  if (!abortArmed.value) { abortArmed.value = true; window.clearTimeout(armTimer); armTimer = window.setTimeout(() => abortArmed.value = false, 3000); return; }
  window.clearTimeout(armTimer); abortArmed.value = false; void store.abandon();
}
onMounted(() => { interval = window.setInterval(() => now.value = Date.now(), 1000); }); onBeforeUnmount(() => { clearInterval(interval); window.clearTimeout(armTimer); });
const elapsed = computed(() => store.running ? Math.max(0, Math.floor((now.value - (store.acceptedUsage[0]?.receivedAt ?? now.value)) / 1000)) : 0);
const clock = computed(() => `${String(Math.floor(elapsed.value / 60)).padStart(2, "0")}:${String(elapsed.value % 60).padStart(2, "0")}`);
const round = computed(() => Math.min(store.maxRounds, Math.max(0, store.currentRound + 1)));
</script>

<template>
  <header class="terminal-panel relative z-20 flex h-14 shrink-0 items-center gap-4 border-x-0 border-t-0 px-4">
    <div class="flex items-center gap-2 text-signal"><Gavel class="size-4" /><span class="font-display text-lg tracking-tight">Krunch</span></div>
    <div class="hidden border-l border-line pl-4 font-mono text-[10px] text-fg-muted xl:block">{{ store.sessionId?.slice(0, 8) ?? "not in session" }} · {{ clock }}</div>
    <div class="font-mono text-[10px] text-fg-muted">{{ store.finalState === 'finalizing' ? 'sealing the record…' : `round ${round}/${store.maxRounds}` }}</div>
    <div class="ml-auto flex items-center gap-3 font-mono text-[10px]">
      <span class="hidden text-fg-muted md:inline"><Cpu class="mr-1 inline size-3 text-consensus" />{{ store.usageSummary.total.toLocaleString() }} tok <b :class="store.usageSummary.exact ? 'text-consensus' : 'text-signal'">{{ store.usageSummary.exact ? 'exact' : 'partial' }}</b></span>
      <span class="hidden text-fg-faint lg:inline"><Gauge class="mr-1 inline size-3" />{{ store.approximateOutputRate ? `~${store.approximateOutputRate.toFixed(1)} tok/s` : '~— tok/s' }}</span>
      <span class="hidden text-fg-muted xl:inline">est. {{ store.estimatedCost === null ? '—' : `$${store.estimatedCost.toFixed(4)}` }}</span>
      <button v-if="store.running" class="border px-2 py-1 transition-colors" :class="abortArmed ? 'border-deadlock bg-deadlock/20 text-deadlock' : 'border-line text-fg-faint hover:border-deadlock hover:text-deadlock'" @click="abort">{{ abortArmed ? 'CONFIRM ABORT' : 'ABORT' }}</button>
      <ToggleGroup :model-value="props.effects" type="single" size="sm" variant="outline" class="hidden border border-line sm:flex" @update:model-value="emit('update:effects', ($event || 'ambient') as 'off' | 'ambient' | 'max')"><ToggleGroupItem value="off">Off</ToggleGroupItem><ToggleGroupItem value="ambient">Ambient</ToggleGroupItem><ToggleGroupItem value="max">Max</ToggleGroupItem></ToggleGroup>
      <button class="border border-line px-2 py-1 text-fg-faint hover:border-signal hover:text-signal" @click="emit('palette')">⌘K</button>
    </div>
  </header>
</template>
