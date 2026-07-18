<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatEditor from "@/components/SeatEditor.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ArrowRight, Plus, Sparkles } from "@lucide/vue";

const store = useDeliberation();

const modeHint = computed(() => ({
  autonomous: "The panel decides alone and never pauses.",
  batched: "The foreman interrupts you only when a question truly matters.",
  interactive: "The panel pauses whenever it has an open question for you.",
}[store.mode]));
</script>

<template>
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-3xl px-8 pb-24 pt-14">
      <!-- Wordmark -->
      <header class="rise flex items-start justify-between gap-6">
        <div class="flex items-center gap-4">
          <div class="relative grid size-14 place-items-center rounded-xl bg-surface-2 ring-1 ring-brass/30"
            style="box-shadow: 0 0 24px -6px color-mix(in oklch, var(--brass) 45%, transparent);">
            <span class="font-display text-2xl text-brass candle">k</span>
          </div>
          <div>
            <h1 class="font-display text-3xl leading-none text-foreground">krunch</h1>
            <p class="mt-1.5 text-sm text-fg-muted">
              Convene a panel of minds. Let them deliberate to a verdict.
            </p>
          </div>
        </div>
        <Button variant="outline" size="sm" class="mt-1 rounded-full border-line text-fg-muted hover:border-brass/50 hover:text-brass"
          @click="store.loadDemoPanel()">
          <Sparkles data-icon="inline-start" />
          Seat a demo panel
        </Button>
      </header>

      <!-- The question -->
      <section class="rise mt-12" style="animation-delay: 60ms">
        <div class="mb-3 flex items-baseline gap-3">
          <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">i.</span>
          <h2 class="font-display text-lg text-foreground">The question before the panel</h2>
        </div>
        <Textarea v-model="store.problem" rows="4" placeholder="State the matter to be deliberated…"
          class="resize-none rounded-lg border-line bg-surface/50 px-5 py-4 text-[15px] leading-relaxed md:text-[15px] focus-visible:border-brass/50 focus-visible:ring-brass/25" />
      </section>

      <!-- Chamber rules -->
      <section class="rise mt-10" style="animation-delay: 120ms">
        <div class="mb-3 flex items-baseline gap-3">
          <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">ii.</span>
          <h2 class="font-display text-lg text-foreground">Rules of the chamber</h2>
        </div>
        <div class="grid grid-cols-[1.7fr_1fr_1fr] items-start gap-4">
          <div class="flex flex-col gap-1.5">
            <Label class="text-fg-muted">When to pause for you</Label>
            <ToggleGroup v-model="store.mode" type="single" variant="outline" class="w-full">
              <ToggleGroupItem value="autonomous" class="flex-1 data-[state=on]:border-brass/50 data-[state=on]:bg-brass/10 data-[state=on]:text-brass">Autonomous</ToggleGroupItem>
              <ToggleGroupItem value="batched" class="flex-1 data-[state=on]:border-brass/50 data-[state=on]:bg-brass/10 data-[state=on]:text-brass">Batched</ToggleGroupItem>
              <ToggleGroupItem value="interactive" class="flex-1 data-[state=on]:border-brass/50 data-[state=on]:bg-brass/10 data-[state=on]:text-brass">Interactive</ToggleGroupItem>
            </ToggleGroup>
            <p class="text-[11px] text-fg-faint">{{ modeHint }}</p>
          </div>
          <div class="flex flex-col gap-1.5">
            <Label class="text-fg-muted">Rounds before deadlock</Label>
            <Input v-model.number="store.maxRounds" type="number" min="1" max="64" class="bg-surface/50" />
          </div>
          <div class="flex flex-col gap-1.5">
            <Label class="text-fg-muted">Confidence to rule</Label>
            <Input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step="0.05" class="bg-surface/50" />
          </div>
        </div>
      </section>

      <!-- The panel -->
      <section class="rise mt-10" style="animation-delay: 180ms">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-baseline gap-3">
            <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">iii.</span>
            <h2 class="font-display text-lg text-foreground">The panel</h2>
          </div>
          <Button variant="outline" size="sm" :disabled="store.panelists.length >= 6"
            class="rounded-full border-line text-fg-muted hover:border-brass/50 hover:text-brass"
            @click="store.addPanelist()">
            <Plus data-icon="inline-start" />
            Seat another
            <span class="font-mono text-fg-faint">{{ store.panelists.length }}/6</span>
          </Button>
        </div>
        <div class="flex flex-col gap-3">
          <SeatEditor v-if="store.mediator" :seat="store.mediator" />
          <SeatEditor v-for="p in store.panelists" :key="p.id" :seat="p" removable @remove="store.removeSeat(p.id)" />
        </div>
      </section>

      <!-- Objections -->
      <Alert v-if="store.validation.length" class="rise mt-8 border-deadlock/25 bg-surface/40">
        <AlertTitle class="font-mono text-[11px] uppercase tracking-[0.15em] text-deadlock">Before you convene</AlertTitle>
        <AlertDescription class="text-fg-muted">
          <ul class="flex flex-col gap-0.5">
            <li v-for="(v, i) in store.validation" :key="i">— {{ v }}</li>
          </ul>
        </AlertDescription>
      </Alert>
      <Alert v-if="store.startError" variant="destructive" class="mt-4">
        <AlertDescription>{{ store.startError }}</AlertDescription>
      </Alert>

      <!-- Convene -->
      <Button size="lg" :disabled="store.validation.length > 0" @click="store.start()"
        class="mt-8 h-14 w-full rounded-xl bg-brass text-primary-foreground hover:bg-brass-bright disabled:bg-surface-3 disabled:text-fg-faint"
        style="box-shadow: 0 8px 40px -12px color-mix(in oklch, var(--brass) 60%, transparent);">
        <span class="font-display text-lg">Convene the panel</span>
        <ArrowRight data-icon="inline-end" />
      </Button>
    </div>
  </div>
</template>
