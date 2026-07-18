<script setup lang="ts">
import { computed } from "vue";
import { renderMarkdown } from "@/lib/markdown";

const props = defineProps<{
  text: string | null | undefined;
  streaming?: boolean;
  cursorClass?: string;
}>();

const html = computed(() => renderMarkdown(props.text));
</script>

<template>
  <div class="markdown-body break-words">
    <div v-html="html" />
    <span v-if="streaming" class="cursor" :class="cursorClass ?? 'text-brass'">▋</span>
  </div>
</template>
