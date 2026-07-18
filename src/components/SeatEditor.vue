<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { SeatConfig } from "@/lib/types";
import { isLoopbackUrl, providerIsHttp } from "@/lib/types";
import { api } from "@/lib/api";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import {
  Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";

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
</script>

<template>
  <Card class="border-line bg-surface/50 gap-0 py-0 transition hover:border-line-strong"
    :class="seat.role === 'mediator' ? 'ring-1 ring-brass/25' : ''">
    <CardHeader class="flex items-center gap-2.5 border-b border-line/60 px-4 py-3 [.border-b]:pb-3">
      <Badge variant="outline" class="font-mono text-[10px] uppercase tracking-[0.14em]"
        :class="seat.role === 'mediator' ? 'border-brass/35 text-brass' : 'border-line text-fg-muted'">
        {{ seat.role === "mediator" ? "at the head" : "seated" }}
      </Badge>
      <input v-model="seat.display_name"
        class="min-w-0 flex-1 border-none bg-transparent font-display text-[15px] text-foreground outline-none" />
      <Button v-if="removable" variant="ghost" size="xs" class="text-fg-faint hover:text-deadlock"
        @click="$emit('remove')">vacate</Button>
    </CardHeader>

    <CardContent class="grid grid-cols-2 gap-x-3 gap-y-3.5 px-4 py-4">
      <div class="flex flex-col gap-1.5">
        <Label class="text-fg-muted">Provider</Label>
        <Select v-model="seat.provider">
          <SelectTrigger class="w-full bg-bg-deep/60"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="anthropic">Anthropic (API key)</SelectItem>
              <SelectItem value="open_ai_compatible">OpenAI-compatible / local</SelectItem>
              <SelectItem value="claude_cli">Claude CLI (subscription)</SelectItem>
              <SelectItem value="codex_cli">Codex CLI (subscription)</SelectItem>
              <SelectItem value="demo">Demo (offline, no key)</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>

      <div v-if="seat.provider !== 'demo'" class="flex flex-col gap-1.5">
        <Label class="text-fg-muted">Model{{ isCli ? " (optional)" : "" }}</Label>
        <Input v-model="seat.model" class="bg-bg-deep/60" />
      </div>

      <div v-if="isHttp" class="col-span-2 flex flex-col gap-1.5">
        <Label class="text-fg-muted">
          Base URL
          <span v-if="isLoopbackUrl(seat.base_url)" class="ml-1 text-consensus">loopback · key-free</span>
        </Label>
        <Input v-model="seat.base_url" class="bg-bg-deep/60" @blur="refreshKey" />
      </div>

      <div class="col-span-2 flex flex-col gap-1.5">
        <Label class="text-fg-muted">System prompt / persona</Label>
        <Textarea v-model="seat.system_prompt" rows="2" class="bg-bg-deep/60 resize-y" />
      </div>

      <p v-if="isCli" class="col-span-2 text-[11px] text-fg-muted">
        Uses your local <code class="font-mono text-foreground">{{ seat.provider === 'claude_cli' ? 'claude' : 'codex' }}</code>
        CLI and its subscription auth — no API key. Make sure it's installed and signed in.
      </p>
      <p v-else-if="seat.provider === 'demo'" class="col-span-2 text-[11px] text-fg-muted">
        Offline demo agent — streams a canned deliberation with no key or network.
      </p>

      <template v-if="needsKey">
        <div class="flex flex-col gap-1.5">
          <Label class="text-fg-muted">Credential ref</Label>
          <Input v-model="seat.credential_ref" class="bg-bg-deep/60" @blur="refreshKey" />
        </div>
        <div class="flex flex-col gap-1.5">
          <Label class="text-fg-muted">
            API key
            <span v-if="keySaved" class="ml-1 text-consensus">● stored</span>
            <span v-else-if="keySaved === false" class="ml-1 text-brass">○ not set</span>
          </Label>
          <div class="flex gap-1.5">
            <Input v-model="keyInput" type="password" placeholder="paste key" class="bg-bg-deep/60" />
            <Button variant="outline" size="sm" :disabled="savingKey || !keyInput" @click="saveKey">save</Button>
          </div>
        </div>
      </template>
    </CardContent>
  </Card>
</template>
