<script setup lang="ts">
import { ref, watch } from "vue";
import { api, isTauri } from "@/lib/api";
import type { SessionDto, SetupSnapshot } from "@/lib/types";
import { useDeliberation } from "@/stores/deliberation";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
import { Dialog, DialogScrollContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

const open = defineModel<boolean>("open", { required: true });

const store = useDeliberation();

const sessions = ref<SessionDto[]>([]);
const selected = ref<SessionDto | null>(null);
const detail = ref<string>("");
const setupRaw = ref<string | null>(null);
const loading = ref(false);

async function refresh() {
  if (!isTauri()) { sessions.value = []; return; }
  try { sessions.value = await api.listSessions(); } catch { sessions.value = []; }
}
async function openSession(s: SessionDto) {
  selected.value = s;
  loading.value = true;
  detail.value = "";
  setupRaw.value = null;
  const [md, raw] = await Promise.allSettled([
    api.exportSession(s.id),
    isTauri() ? api.getSessionSetup(s.id) : Promise.resolve(null),
  ]);
  if (selected.value?.id !== s.id) return; // a newer selection superseded this one
  detail.value = md.status === "fulfilled" ? md.value : `_Could not load this session: ${String(md.reason)}_`;
  setupRaw.value = raw.status === "fulfilled" ? raw.value : null;
  loading.value = false;
}

function cloneAsNew() {
  if (!setupRaw.value) return;
  try {
    const snap = JSON.parse(setupRaw.value) as SetupSnapshot;
    store.hydrateSetup(snap, { problem: true }); // load problem + roster into the setup editor
    open.value = false; // dialog only opens from the setup phase, so we land back on setup
  } catch { /* corrupt snapshot — leave the editor untouched */ }
}

// Refresh the list each time the dialog opens; reset the detail pane.
watch(open, (isOpen) => { if (isOpen) { selected.value = null; detail.value = ""; void refresh(); } });

function fmt(ts: number): string { return new Date(ts).toLocaleString(); }
</script>

<template>
  <Dialog v-model:open="open">
    <DialogScrollContent class="max-w-4xl">
      <DialogHeader>
        <DialogTitle class="font-display text-signal">Past deliberations</DialogTitle>
        <DialogDescription class="font-mono text-[10px] uppercase tracking-[0.14em]">Read-only review of stored sessions.</DialogDescription>
      </DialogHeader>
      <div class="grid min-h-0 gap-4 md:grid-cols-[18rem_minmax(0,1fr)]">
        <ul class="max-h-[60vh] space-y-1 overflow-y-auto border-r border-line pr-3 font-mono text-[11px]">
          <li v-if="!sessions.length" class="text-fg-faint">No stored sessions.</li>
          <li v-for="s in sessions" :key="s.id">
            <button
              class="w-full rounded px-2 py-1.5 text-left hover:bg-surface"
              :class="selected?.id === s.id ? 'bg-surface text-signal' : 'text-fg-muted'"
              @click="openSession(s)"
            >
              <span class="line-clamp-2">{{ s.problem || "(untitled)" }}</span>
              <span class="mt-0.5 block text-[10px] text-fg-faint">{{ s.state }} · {{ fmt(s.created_at) }}</span>
            </button>
          </li>
        </ul>
        <div class="max-h-[60vh] overflow-y-auto">
          <p v-if="!selected" class="font-mono text-[11px] text-fg-faint">Select a session to review it.</p>
          <p v-else-if="loading" class="font-mono text-[11px] text-fg-faint">Loading…</p>
          <template v-else>
            <div class="mb-3 flex items-center gap-3">
              <Button size="sm" variant="outline" class="border-consensus/45 text-consensus" :disabled="!setupRaw" @click="cloneAsNew">Start new from this</Button>
              <span v-if="!setupRaw" class="font-mono text-[10px] text-fg-faint">setup not captured for this session</span>
            </div>
            <StreamMarkdown :text="detail" />
          </template>
        </div>
      </div>
    </DialogScrollContent>
  </Dialog>
</template>
