<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useIntersectionObserver } from "@vueuse/core";
import { Plus, Play, Sparkles } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatRoster from "@/components/SeatRoster.vue";
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

// Seats in bench order: the mediator first, then panelists.
const orderedSeats = computed(() => (store.mediator ? [store.mediator, ...store.panelists] : [...store.panelists]));
const selectedSeatId = ref<string | null>(store.mediator?.id ?? null);
const selectedSeat = computed(() => store.seats.find((seat) => seat.id === selectedSeatId.value) ?? store.mediator ?? null);

// Selection must follow store mutations that bypass the component: the `A`
// shortcut adds a seat straight through the store, and loadDemoPanel replaces
// every seat. Watch the id list — an added id gets selected; if the current
// selection vanished (removed / demo-replaced), fall back to the mediator.
watch(() => store.seats.map((seat) => seat.id), (ids, prev) => {
  const added = ids.find((id) => !prev?.includes(id));
  if (added) { selectedSeatId.value = added; return; }
  if (!selectedSeatId.value || !ids.includes(selectedSeatId.value)) selectedSeatId.value = store.mediator?.id ?? null;
});

function selectSeat(id: string) { selectedSeatId.value = id; }
function removeSelected() {
  const id = selectedSeatId.value;
  if (!id) return;
  selectedSeatId.value = store.mediator?.id ?? null; // reselect before removal so no empty state flashes
  store.removeSeat(id);
}

// Sticky convene bar reveals once the hero convene card scrolls out of view.
const scrollRoot = ref<HTMLElement | null>(null);
const heroCard = ref<HTMLElement | null>(null);
const heroVisible = ref(true);
useIntersectionObserver(heroCard, ([entry]) => { heroVisible.value = entry.isIntersecting; }, { root: scrollRoot });
</script>

<template>
  <main class="min-h-0 flex-1 overflow-y-auto p-4 lg:p-6">
    <div class="mx-auto grid max-w-[110rem] grid-cols-[minmax(0,1fr)_19rem] gap-6">
      <div class="min-w-0 space-y-5">
        <header class="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-4">
          <div><h1 class="font-display text-3xl text-foreground">Convene the panel</h1><p class="mt-2 text-sm text-fg-muted">State the matter, seat the panel, then open deliberation.</p></div>
          <Button size="sm" variant="outline" class="border-brass/50 text-brass" @click="store.loadDemoPanel()"><Sparkles data-icon="inline-start" />Load demo panel</Button>
        </header>
        <section class="terminal-panel p-5"><p class="mb-2.5 font-mono text-[11px] uppercase tracking-[0.14em] text-brass">The matter</p><Textarea v-model="store.problem" rows="4" placeholder="State the matter to deliberate…" class="resize-none bg-bg-deep text-sm leading-relaxed" /></section>
        <section class="terminal-panel grid grid-cols-[1.5fr_repeat(3,minmax(0,1fr))] gap-4 p-4"><label class="font-mono text-[10px] text-fg-muted">INTERACTION MODE<ToggleGroup v-model="store.mode" type="single" variant="outline" class="mt-1.5 grid grid-cols-3"><ToggleGroupItem value="autonomous">AUTO</ToggleGroupItem><ToggleGroupItem value="batched">BATCH</ToggleGroupItem><ToggleGroupItem value="interactive">LIVE</ToggleGroupItem></ToggleGroup><span class="mt-1.5 block text-[9px] text-fg-faint">{{ modeHint }}</span></label><label class="font-mono text-[10px] text-fg-muted">MAX ROUNDS<Input v-model.number="store.maxRounds" type="number" min="1" max="64" class="mt-1.5 bg-bg-deep" /></label><label class="font-mono text-[10px] text-fg-muted">QUORUM<Input v-model.number="quorumDisplay" type="number" min="0" max="1" step=".05" class="mt-1.5 bg-bg-deep" /></label><label class="font-mono text-[10px] text-fg-muted">CONFIDENCE<Input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step=".05" class="mt-1.5 bg-bg-deep" /></label></section>
        <section class="grid gap-4 lg:grid-cols-[21rem_minmax(0,1fr)]">
          <div>
            <header class="mb-3 flex items-center justify-between">
              <p class="font-mono text-[11px] uppercase tracking-[0.14em] text-brass">The panel // {{ store.panelists.length }}/6 seated</p>
              <Button size="xs" variant="outline" :disabled="store.panelists.length >= 6" class="border-consensus/45 text-consensus" @click="store.addPanelist()"><Plus data-icon="inline-start" />Add seat <kbd class="ml-1 text-fg-faint">A</kbd></Button>
            </header>
            <SeatRoster :seats="orderedSeats" :selected-id="selectedSeatId" :can-add="store.panelists.length < 6" @select="selectSeat" @add="store.addPanelist()" />
          </div>
          <SeatEditor v-if="selectedSeat" :key="selectedSeat.id" :seat="selectedSeat" :removable="selectedSeat.role !== 'mediator'" @remove="removeSelected" />
        </section>
      </div>
      <aside class="sticky top-4 self-start terminal-panel p-4">
        <p class="font-mono text-[11px] uppercase tracking-[0.14em] text-brass">Readiness</p>
        <dl class="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 font-mono text-[10px]"><dt class="text-fg-faint">panel</dt><dd class="text-right text-fg-muted">1 med · {{ store.panelists.length }} {{ store.panelists.length === 1 ? 'seat' : 'seats' }}</dd><dt class="text-fg-faint">mode</dt><dd class="text-right text-fg-muted">{{ store.mode }}</dd><dt class="text-fg-faint">rounds</dt><dd class="text-right text-fg-muted">max {{ store.maxRounds }}</dd></dl>
        <ul class="mt-4 space-y-2 border-t border-line pt-4 font-mono text-[10px]"><li v-if="!store.validation.length" class="text-consensus">[✓] ready to convene</li><li v-for="item in store.validation" :key="item" class="text-deadlock">[!] {{ item }}</li></ul>
        <p v-if="store.startError" class="mt-3 border-t border-deadlock/40 pt-3 text-[10px] text-deadlock">{{ store.startError }}</p>
        <Button class="mt-5 w-full bg-consensus font-mono text-primary-foreground hover:bg-consensus/85" :disabled="Boolean(store.validation.length)" @click="store.start()"><Play data-icon="inline-start" />Convene panel <kbd class="ml-1 opacity-70">C</kbd></Button>
      </aside>
    </div>
  </main>
</template>
