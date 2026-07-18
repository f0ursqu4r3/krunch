<script setup lang="ts">
import { onMounted } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import SetupScreen from "@/screens/SetupScreen.vue";
import RoomScreen from "@/screens/RoomScreen.vue";
import VerdictScreen from "@/screens/VerdictScreen.vue";

const store = useDeliberation();
onMounted(() => store.init());
</script>

<template>
  <div class="h-full w-full overflow-hidden">
    <SetupScreen v-if="store.phase === 'setup'" />
    <RoomScreen v-else-if="store.phase === 'room'" />
    <VerdictScreen v-else />
  </div>
</template>
