<script setup lang="ts">
import { ref, watch } from "vue";
import { api, isTauri } from "@/lib/api";
import type { SessionDto } from "@/lib/types";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
import { Dialog, DialogScrollContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";

const open = defineModel<boolean>("open", { required: true });

const sessions = ref<SessionDto[]>([]);
const selected = ref<SessionDto | null>(null);
const detail = ref<string>("");
const loading = ref(false);

async function refresh() {
  if (!isTauri()) { sessions.value = []; return; }
  try { sessions.value = await api.listSessions(); } catch { sessions.value = []; }
}
async function openSession(s: SessionDto) {
  selected.value = s;
  loading.value = true;
  detail.value = "";
  try { detail.value = await api.exportSession(s.id); }
  catch (e) { detail.value = `_Could not load this session: ${String(e)}_`; }
  finally { loading.value = false; }
}

// Refresh the list each time the dialog opens; reset the detail pane.
watch(open, (isOpen) => { if (isOpen) { selected.value = null; detail.value = ""; void refresh(); } });

function fmt(ts: number): string { return new Date(ts).toLocaleString(); }
</script>

<template>
  <Dialog v-model:open="open">
    <DialogScrollContent class="max-w-4xl">
      <DialogHeader>
        <DialogTitle class="font-display text-brass">Past deliberations</DialogTitle>
        <DialogDescription class="font-mono text-[10px] uppercase tracking-[0.14em]">Read-only review of stored sessions.</DialogDescription>
      </DialogHeader>
      <div class="grid min-h-0 gap-4 md:grid-cols-[18rem_minmax(0,1fr)]">
        <ul class="max-h-[60vh] space-y-1 overflow-y-auto border-r border-line pr-3 font-mono text-[11px]">
          <li v-if="!sessions.length" class="text-fg-faint">No stored sessions.</li>
          <li v-for="s in sessions" :key="s.id">
            <button
              class="w-full rounded px-2 py-1.5 text-left hover:bg-surface"
              :class="selected?.id === s.id ? 'bg-surface text-brass' : 'text-fg-muted'"
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
          <StreamMarkdown v-else :text="detail" />
        </div>
      </div>
    </DialogScrollContent>
  </Dialog>
</template>
