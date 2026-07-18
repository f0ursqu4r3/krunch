<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { SeatConfig } from "@/lib/types";
import { isLoopbackUrl, providerIsHttp } from "@/lib/types";
import { api } from "@/lib/api";

const props = defineProps<{ seat: SeatConfig; removable?: boolean }>();
defineEmits<{ remove: [] }>();

const isHttp = computed(() => providerIsHttp(props.seat.provider));
const needsKey = computed(() => isHttp.value && !isLoopbackUrl(props.seat.base_url));
const isCli = computed(
  () => props.seat.provider === "claude_cli" || props.seat.provider === "codex_cli",
);

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
  "w-full rounded-md border border-line bg-bg-deep/60 px-2.5 py-1.5 text-sm text-foreground outline-none transition focus:border-brass/60 focus:ring-1 focus:ring-brass/40";
</script>

<template>
  <div class="rounded-xl border border-line bg-surface/50 p-4 transition hover:border-line-strong"
    :class="seat.role === 'mediator' ? 'ring-1 ring-brass/25' : ''">
    <div class="mb-3.5 flex items-center justify-between">
      <div class="flex items-center gap-2.5">
        <span class="rounded px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.14em] ring-1"
          :class="seat.role === 'mediator'
            ? 'bg-brass/12 text-brass ring-brass/35'
            : 'bg-surface-3 text-fg-muted ring-line'">
          {{ seat.role === "mediator" ? "at the head" : "seated" }}
        </span>
        <input v-model="seat.display_name"
          class="border-none bg-transparent font-display text-[15px] text-foreground outline-none" />
      </div>
      <button v-if="removable" @click="$emit('remove')"
        class="text-xs text-fg-faint transition hover:text-deadlock">
        vacate
      </button>
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="col-span-1 text-xs">
        <span class="mb-1 block text-muted-foreground">Provider</span>
        <select v-model="seat.provider" :class="inputCls">
          <option value="anthropic">Anthropic (API key)</option>
          <option value="open_ai_compatible">OpenAI-compatible / local</option>
          <option value="claude_cli">Claude CLI (subscription)</option>
          <option value="codex_cli">Codex CLI (subscription)</option>
          <option value="demo">Demo (offline, no key)</option>
        </select>
      </label>
      <label class="col-span-1 text-xs" v-if="seat.provider !== 'demo'">
        <span class="mb-1 block text-muted-foreground">Model{{ isCli ? " (optional)" : "" }}</span>
        <input v-model="seat.model" :class="inputCls" />
      </label>
      <label class="col-span-2 text-xs" v-if="isHttp">
        <span class="mb-1 block text-muted-foreground">
          Base URL
          <span v-if="isLoopbackUrl(seat.base_url)" class="ml-1 text-consensus">loopback · key-free</span>
        </span>
        <input v-model="seat.base_url" :class="inputCls" @blur="refreshKey" />
      </label>
      <label class="col-span-2 text-xs">
        <span class="mb-1 block text-muted-foreground">System prompt / persona</span>
        <textarea v-model="seat.system_prompt" rows="2" :class="inputCls" />
      </label>

      <p v-if="isCli" class="col-span-2 text-[11px] text-muted-foreground">
        Uses your local <code class="text-foreground">{{ seat.provider === 'claude_cli' ? 'claude' : 'codex' }}</code>
        CLI and its subscription auth — no API key. Make sure it's installed and signed in.
      </p>
      <p v-else-if="seat.provider === 'demo'" class="col-span-2 text-[11px] text-muted-foreground">
        Offline demo agent — streams a canned deliberation with no key or network.
      </p>

      <template v-if="needsKey">
        <label class="col-span-1 text-xs">
          <span class="mb-1 block text-muted-foreground">Credential ref</span>
          <input v-model="seat.credential_ref" :class="inputCls" @blur="refreshKey" />
        </label>
        <div class="col-span-1 text-xs">
          <span class="mb-1 block text-muted-foreground">
            API key
            <span v-if="keySaved" class="ml-1 text-consensus">● stored</span>
            <span v-else-if="keySaved === false" class="ml-1 text-brass">○ not set</span>
          </span>
          <div class="flex gap-1">
            <input v-model="keyInput" type="password" placeholder="paste key" :class="inputCls" />
            <button @click="saveKey" :disabled="savingKey || !keyInput"
              class="shrink-0 rounded-lg border px-2 text-xs hover:bg-accent disabled:opacity-40">
              save
            </button>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
