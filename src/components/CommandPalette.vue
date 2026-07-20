<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { ListboxItem, ListboxRoot } from "reka-ui";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useDeliberation } from "@/stores/deliberation";
import type { AppPhase, ShortcutAction } from "@/lib/shortcuts";

const props = defineProps<{ open: boolean; phase: AppPhase }>();
const emit = defineEmits<{ "update:open": [open: boolean]; action: [action: ShortcutAction] }>();
const store = useDeliberation();
const query = ref("");
const selected = ref<string>();
const input = ref<InstanceType<typeof Input>>();
const entries = computed(() => [
  { id: "convene", label: "Convene panel", keys: "C", show: props.phase === "setup" },
  { id: "add-seat", label: "Add panelist", keys: "A", show: props.phase === "setup" },
  { id: "abort", label: "Abort deliberation", keys: "", show: props.phase === "room" && store.running },
  { id: "new-session", label: "New session", keys: props.phase === "verdict" ? "N" : "", show: props.phase !== "setup" },
  { id: "export", label: "Export session dump", keys: "E", show: props.phase === "verdict" },
  { id: "help", label: "Show shortcuts", keys: "?", show: true },
].filter((entry) => entry.show && entry.label.toLowerCase().includes(query.value.toLowerCase())));
watch(() => props.open, async (open) => { if (open) { query.value = ""; selected.value = undefined; await nextTick(); input.value?.$el?.focus(); } });
function choose(value: unknown) { if (typeof value !== "string") return; emit("action", value as ShortcutAction); emit("update:open", false); }
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent :show-close-button="false" class="max-w-xl border-brass/50 bg-surface-2 p-0" @escape-key-down="emit('update:open', false)">
      <DialogTitle class="sr-only">Command palette</DialogTitle><DialogDescription class="sr-only">Run a command.</DialogDescription>
      <div class="border-b border-line p-3"><Input ref="input" v-model="query" placeholder="Search commands…" class="border-0 bg-transparent text-sm focus-visible:ring-0" /></div>
      <ListboxRoot v-model="selected" class="p-2" @update:model-value="choose">
        <ListboxItem v-for="entry in entries" :key="entry.id" :value="entry.id" as="button" class="flex w-full items-center justify-between px-3 py-2 text-left font-mono text-xs text-fg-muted outline-none data-[highlighted]:bg-brass/15 data-[highlighted]:text-brass">
          <span>{{ entry.label }}</span><kbd v-if="entry.keys" class="border border-line px-1.5 py-0.5 text-[9px] text-fg-faint">{{ entry.keys }}</kbd>
        </ListboxItem>
        <p v-if="!entries.length" class="p-3 font-mono text-xs text-fg-faint">no command matches</p>
      </ListboxRoot>
      <footer class="border-t border-line px-4 py-2 font-mono text-[9px] text-fg-faint">↑↓ navigate · enter run · esc close</footer>
    </DialogContent>
  </Dialog>
</template>
