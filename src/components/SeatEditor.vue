<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import type { SeatConfig } from "@/lib/types";
import type { PersonaGroup } from "@/lib/personas";
import { isLoopbackUrl, providerIsHttp } from "@/lib/types";
import { NONE, personaById, personasForGroup, groupsForRole } from "@/lib/personas";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button"; import { Input } from "@/components/ui/input"; import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
const props = defineProps<{ seat: SeatConfig; removable?: boolean }>(); defineEmits<{ remove: [] }>();
const http = computed(() => providerIsHttp(props.seat.provider)); const needsKey = computed(() => http.value && !isLoopbackUrl(props.seat.base_url)); const key = ref(""); const saved = ref<boolean | null>(null);
// One proxy per persona group: reads/writes the single id of that group inside
// seat.personas, using NONE as the "no persona" sentinel.
function groupProxy(group: PersonaGroup) {
  return computed<string>({
    get: () => props.seat.personas.find((id) => personaById(id)?.group === group) ?? NONE,
    set: (id: string) => {
      const others = props.seat.personas.filter((pid) => personaById(pid)?.group !== group);
      props.seat.personas = id === NONE ? others : [...others, id];
    },
  });
}
const temperament = groupProxy("temperament");
const domain = groupProxy("domain");
const mediatorPersona = groupProxy("mediator");

const temperamentOptions = personasForGroup("temperament");
const domainOptions = personasForGroup("domain");
const mediatorOptions = personasForGroup("mediator");

// When a seat's role changes, drop persona ids whose group is invalid for it.
watch(() => props.seat.role, (role) => {
  const allowed = groupsForRole(role);
  props.seat.personas = props.seat.personas.filter((id) => {
    const g = personaById(id)?.group;
    return g !== undefined && allowed.includes(g);
  });
});
async function refresh() { try { saved.value = await api.hasCredential(props.seat.credential_ref); } catch { saved.value = null; } }
async function save() { if (!key.value) return; await api.setCredential(props.seat.credential_ref, key.value); key.value = ""; await refresh(); } onMounted(refresh);
</script>

<template>
  <article class="border border-line bg-bg-deep/35 p-3"><header class="mb-3 flex items-center gap-2 border-b border-line pb-2"><span class="font-mono text-[9px]" :class="seat.role === 'mediator' ? 'text-cyan' : 'text-fg-faint'">{{ seat.role === 'mediator' ? 'MED' : 'SEAT' }}</span><input v-model="seat.display_name" class="min-w-0 flex-1 bg-transparent font-display text-xs text-foreground outline-none" /><Button v-if="removable" size="xs" variant="ghost" class="text-deadlock" @click="$emit('remove')">remove</Button></header><div class="grid grid-cols-2 gap-2 font-mono text-[9px] text-fg-muted"><label>PROVIDER<Select v-model="seat.provider"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="anthropic">Anthropic</SelectItem><SelectItem value="open_ai_compatible">OpenAI-compatible</SelectItem><SelectItem value="claude_cli">Claude CLI</SelectItem><SelectItem value="codex_cli">Codex CLI</SelectItem><SelectItem value="demo">Demo</SelectItem></SelectGroup></SelectContent></Select></label><label v-if="seat.provider !== 'demo'">MODEL<Input v-model="seat.model" class="mt-1 h-8 bg-surface" /></label><label v-if="http" class="col-span-2">BASE URL<Input v-model="seat.base_url" class="mt-1 h-8 bg-surface" @blur="refresh" /></label><template v-if="seat.role !== 'mediator'"><label>TEMPERAMENT<Select v-model="temperament"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in temperamentOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label><label>DOMAIN EXPERT<Select v-model="domain"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in domainOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label></template><label v-else class="col-span-2">MEDIATOR PERSONA<Select v-model="mediatorPersona"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in mediatorOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label><label class="col-span-2">CUSTOM (ADDENDUM)<Textarea v-model="seat.system_prompt" rows="2" placeholder="optional extra instructions, appended after the persona" class="mt-1 resize-y bg-surface text-[10px]" /></label><label>TEMPERATURE<Input v-model.number="seat.sampling.temperature" type="number" step=".05" min="0" max="2" class="mt-1 h-8 bg-surface" /></label><label>TOP P<Input v-model.number="seat.sampling.top_p" type="number" step=".05" min="0" max="1" class="mt-1 h-8 bg-surface" /></label><label>MAX TOKENS<Input v-model.number="seat.sampling.max_tokens" type="number" min="1" class="mt-1 h-8 bg-surface" /></label><template v-if="needsKey"><label>CREDENTIAL REF<Input v-model="seat.credential_ref" class="mt-1 h-8 bg-surface" @blur="refresh" /></label><label>API KEY {{ saved ? '[stored]' : '' }}<div class="mt-1 flex gap-1"><Input v-model="key" type="password" class="h-8 bg-surface" /><Button size="xs" variant="outline" @click="save">save</Button></div></label></template><p v-if="seat.provider === 'demo'" class="col-span-2 text-consensus">offline demo agent, no key required</p><p v-if="seat.provider === 'claude_cli' || seat.provider === 'codex_cli'" class="col-span-2 text-brass">uses local subscription CLI</p></div></article>
</template>
