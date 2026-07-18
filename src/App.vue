<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// M1 health check: prove the Vue ↔ Rust ↔ krunch-core bridge is wired.
const coreVersion = ref<string>("…");
const bridgeError = ref<string | null>(null);

onMounted(async () => {
  try {
    coreVersion.value = await invoke<string>("core_version");
  } catch (e) {
    bridgeError.value = String(e);
  }
});
</script>

<template>
  <main class="flex h-full w-full items-center justify-center p-8">
    <div class="max-w-md text-center">
      <div
        class="mx-auto mb-6 flex size-16 items-center justify-center rounded-2xl bg-primary/15 text-2xl font-bold text-primary ring-1 ring-primary/30"
      >
        k
      </div>
      <h1 class="text-3xl font-semibold tracking-tight">krunch</h1>
      <p class="mt-2 text-sm text-muted-foreground">
        A jury room for a panel of LLMs — deliberate until consensus or deadlock.
      </p>

      <div
        class="mt-8 rounded-lg border bg-card px-4 py-3 text-left text-sm text-card-foreground"
      >
        <div class="flex items-center justify-between">
          <span class="text-muted-foreground">core bridge</span>
          <span v-if="bridgeError" class="font-mono text-destructive">error</span>
          <span v-else class="font-mono text-primary">v{{ coreVersion }}</span>
        </div>
        <p v-if="bridgeError" class="mt-2 font-mono text-xs text-destructive">
          {{ bridgeError }}
        </p>
      </div>

      <p class="mt-6 text-xs text-muted-foreground">
        M1 scaffold · setup → room → verdict UI arrives in M6
      </p>
    </div>
  </main>
</template>
