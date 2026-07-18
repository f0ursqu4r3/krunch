<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { SeatConfig } from "@/lib/types";
import { api } from "@/lib/api";

const props = defineProps<{ seat: SeatConfig; removable?: boolean }>();
defineEmits<{ remove: [] }>();

const keyInput = ref("");
const keySaved = ref<boolean | null>(null);
const savingKey = ref(false);

async function refreshKey() {
  try {
    keySaved.value = await api.hasCredential(props.seat.credential_ref);
  } catch {
    keySaved.value = null;
  }
}
async function saveKey() {
  if (!keyInput.value) return;
  savingKey.value = true;
  try {
    await api.setCredential(props.seat.credential_ref, keyInput.value);
    keyInput.value = "";
    await refreshKey();
  } finally {
    savingKey.value = false;
  }
}
onMounted(refreshKey);

const inputCls =
  "w-full rounded-lg border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-2 focus:ring-ring";
</script>

<template>
  <div class="rounded-xl border bg-card p-4">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <span class="rounded-md px-2 py-0.5 text-[11px] font-semibold ring-1"
          :class="seat.role === 'mediator'
            ? 'bg-primary/15 text-primary ring-primary/30'
            : 'bg-muted text-muted-foreground ring-border'">
          {{ seat.role === "mediator" ? "Mediator" : "Panelist" }}
        </span>
        <input v-model="seat.display_name" class="border-none bg-transparent text-sm font-medium outline-none" />
      </div>
      <button v-if="removable" @click="$emit('remove')" class="text-xs text-muted-foreground hover:text-destructive">
        remove
      </button>
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="col-span-1 text-xs">
        <span class="mb-1 block text-muted-foreground">Provider</span>
        <select v-model="seat.provider" :class="inputCls">
          <option value="anthropic">Anthropic</option>
          <option value="open_ai_compatible">OpenAI-compatible</option>
        </select>
      </label>
      <label class="col-span-1 text-xs">
        <span class="mb-1 block text-muted-foreground">Model</span>
        <input v-model="seat.model" :class="inputCls" />
      </label>
      <label class="col-span-2 text-xs">
        <span class="mb-1 block text-muted-foreground">Base URL</span>
        <input v-model="seat.base_url" :class="inputCls" />
      </label>
      <label class="col-span-2 text-xs">
        <span class="mb-1 block text-muted-foreground">System prompt / persona</span>
        <textarea v-model="seat.system_prompt" rows="2" :class="inputCls" />
      </label>
      <label class="col-span-1 text-xs">
        <span class="mb-1 block text-muted-foreground">Credential ref</span>
        <input v-model="seat.credential_ref" :class="inputCls" @blur="refreshKey" />
      </label>
      <div class="col-span-1 text-xs">
        <span class="mb-1 block text-muted-foreground">
          API key
          <span v-if="keySaved" class="ml-1 text-emerald-400">● stored</span>
          <span v-else-if="keySaved === false" class="ml-1 text-amber-400">○ not set</span>
        </span>
        <div class="flex gap-1">
          <input v-model="keyInput" type="password" placeholder="paste key" :class="inputCls" />
          <button @click="saveKey" :disabled="savingKey || !keyInput"
            class="shrink-0 rounded-lg border px-2 text-xs hover:bg-accent disabled:opacity-40">
            save
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
