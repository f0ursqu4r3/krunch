<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Save, Trash2 } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import { useSettings } from "@/stores/settings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { SetupSnapshot } from "@/lib/types";

const store = useDeliberation();
const settings = useSettings();
const name = ref("");

onMounted(() => settings.loadPresets());

async function save() {
  const n = name.value.trim();
  if (!n) return;
  const existing = settings.presets.find((p) => p.name === n);
  if (existing && !window.confirm(`Overwrite preset "${n}"?`)) return;
  await settings.savePreset(n, store.snapshotSetup(false)); // roster + rules, no problem
  name.value = "";
}
function load(configJson: string) {
  try {
    const snap = JSON.parse(configJson) as SetupSnapshot;
    store.hydrateSetup(snap, { problem: false }); // keep the current problem text
  } catch { /* corrupt preset — ignore */ }
}
async function remove(id: string) { await settings.removePreset(id); }
</script>

<template>
  <div class="terminal-panel p-4">
    <p class="mb-2.5 font-mono text-[11px] uppercase tracking-[0.14em] text-brass">Panel presets</p>
    <div class="flex gap-2">
      <Input v-model="name" placeholder="Name this panel…" class="h-8 bg-bg-deep text-[11px]" @keydown.enter="save" />
      <Button size="xs" variant="outline" class="border-brass/50 text-brass" :disabled="!name.trim()" @click="save"><Save data-icon="inline-start" />Save</Button>
    </div>
    <ul v-if="settings.presets.length" class="mt-3 space-y-1.5 border-t border-line pt-3 font-mono text-[11px]">
      <li v-for="p in settings.presets" :key="p.id" class="flex items-center gap-2">
        <button class="flex-1 truncate text-left text-fg-muted hover:text-brass" @click="load(p.config_json)">{{ p.name }}</button>
        <button class="text-fg-faint hover:text-deadlock" :aria-label="`Delete ${p.name}`" @click="remove(p.id)"><Trash2 class="size-3.5" /></button>
      </li>
    </ul>
    <p v-else class="mt-3 border-t border-line pt-3 font-mono text-[10px] text-fg-faint">No presets yet. Seat a panel, then save it.</p>
  </div>
</template>
