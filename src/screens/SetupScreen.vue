<script setup lang="ts">
import { computed } from "vue";
import { Plus, Play, Sparkles } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatEditor from "@/components/SeatEditor.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

const store = useDeliberation();
const modeHint = computed(() => ({ autonomous: "no operator pauses", batched: "pause only for unresolved questions", interactive: "pause for each open question" }[store.mode]));
// Display the quorum rounded to 2dp; keep the exact stored fraction (e.g. 2/3)
// so the consensus guard's `cluster_fraction >= quorum_fraction` is unchanged.
const quorumDisplay = computed({
  get: () => Number(store.quorumFraction.toFixed(2)),
  set: (value: number) => { store.quorumFraction = value; },
});
</script>

<template>
  <main class="min-h-0 flex-1 overflow-y-auto p-4">
    <div class="mx-auto grid max-w-[110rem] grid-cols-[minmax(0,1fr)_18rem] gap-3">
      <section class="space-y-3">
        <header class="terminal-panel flex items-center justify-between gap-4 p-4"><div><p class="font-display text-xl text-cyan">KRUNCH MISSION CONTROL</p><p class="mt-1 font-mono text-[10px] text-fg-muted">configure the panel, then convene a live deliberation</p></div><Button size="sm" variant="outline" class="border-cyan/50 text-cyan" @click="store.loadDemoPanel()"><Sparkles data-icon="inline-start" />Load demo panel</Button></header>
        <section class="terminal-panel p-4"><p class="mb-2 font-mono text-xs text-cyan">MISSION BRIEF</p><Textarea v-model="store.problem" rows="4" placeholder=">_ state the matter to deliberate" class="resize-none bg-bg-deep font-mono text-sm" /></section>
        <section class="terminal-panel grid grid-cols-[1.5fr_repeat(3,minmax(0,1fr))] gap-3 p-4"><label class="font-mono text-[10px] text-fg-muted">INTERACTION MODE<ToggleGroup v-model="store.mode" type="single" variant="outline" class="mt-1 grid grid-cols-3"><ToggleGroupItem value="autonomous">AUTO</ToggleGroupItem><ToggleGroupItem value="batched">BATCH</ToggleGroupItem><ToggleGroupItem value="interactive">LIVE</ToggleGroupItem></ToggleGroup><span class="mt-1 block text-[9px] text-fg-faint">{{ modeHint }}</span></label><label class="font-mono text-[10px] text-fg-muted">MAX ROUNDS<Input v-model.number="store.maxRounds" type="number" min="1" max="64" class="mt-1 bg-bg-deep" /></label><label class="font-mono text-[10px] text-fg-muted">QUORUM<Input v-model.number="quorumDisplay" type="number" min="0" max="1" step=".05" class="mt-1 bg-bg-deep" /></label><label class="font-mono text-[10px] text-fg-muted">CONFIDENCE<Input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step=".05" class="mt-1 bg-bg-deep" /></label></section>
        <section class="terminal-panel p-4"><header class="mb-3 flex items-center justify-between"><p class="font-mono text-xs text-cyan">SEAT ARRAY // {{ store.panelists.length }}/6 PANELISTS</p><Button size="xs" variant="outline" :disabled="store.panelists.length >= 6" class="border-consensus/45 text-consensus" @click="store.addPanelist()"><Plus data-icon="inline-start" />Add seat <kbd class="ml-1 text-fg-faint">A</kbd></Button></header><div class="grid gap-3 xl:grid-cols-2"><SeatEditor v-if="store.mediator" :seat="store.mediator" /><SeatEditor v-for="seat in store.panelists" :key="seat.id" :seat="seat" removable @remove="store.removeSeat(seat.id)" /></div></section>
      </section>
      <aside class="terminal-panel h-fit p-4"><p class="font-mono text-xs text-brass">PREFLIGHT</p><ul class="mt-3 space-y-2 font-mono text-[10px]"><li v-if="!store.validation.length" class="text-consensus">[✓] all systems ready</li><li v-for="item in store.validation" :key="item" class="text-deadlock">[!] {{ item }}</li></ul><p v-if="store.startError" class="mt-3 border-t border-deadlock/40 pt-3 text-[10px] text-deadlock">{{ store.startError }}</p><Button class="mt-5 w-full bg-consensus font-mono text-primary-foreground hover:bg-consensus/85" :disabled="Boolean(store.validation.length)" @click="store.start()"><Play data-icon="inline-start" />Convene panel <kbd class="ml-1 opacity-70">C</kbd></Button></aside>
    </div>
  </main>
</template>
