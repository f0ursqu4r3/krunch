<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import ConvergenceStrip from "@/components/ConvergenceStrip.vue";

const store = useDeliberation();
const canvas = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;
let raf = 0;
let phase = 0;
let ro: ResizeObserver | null = null;

// The live convergence signal — same data the retired glow computed read.
const signal = computed(() => {
  const c = store.convergence;
  const seats = store.panelists.map((p) => store.live[p.id]).filter(Boolean);
  const confs = seats.map((s) => s?.confidence ?? 0);
  const mean = c?.meanConfidence ?? (confs.length ? confs.reduce((a, b) => a + b, 0) / confs.length : 0);
  // Majority stance → phase-lock group; others ride out of phase.
  const counts = new Map<string, number>();
  for (const s of seats) if (s?.stance) counts.set(s.stance, (counts.get(s.stance) ?? 0) + 1);
  let majority = ""; let best = 0;
  for (const [k, v] of counts) if (v > best) { best = v; majority = k; }
  return {
    ruling: c?.effectiveRuling ?? "CONTINUE",
    cluster: c?.clusterFraction ?? 0,
    mean,
    channels: seats.map((s, i) => ({
      amp: 0.15 + (s?.confidence ?? 0) * 0.85,
      streaming: s?.status === "streaming",
      inGroup: !!s?.stance && s.stance === majority,
      idx: i,
    })),
  };
});

function color(): { line: string; glow: string } {
  const r = signal.value.ruling;
  if (r === "DEADLOCK") return { line: "oklch(0.62 0.25 22)", glow: "oklch(0.62 0.25 22 / 0.5)" };
  if (r === "CONSENSUS") return { line: "oklch(0.90 0.24 158)", glow: "oklch(0.90 0.24 158 / 0.6)" };
  return { line: "oklch(0.85 0.21 160)", glow: "oklch(0.85 0.21 160 / 0.4)" };
}

function resize() {
  const cv = canvas.value; if (!cv) return;
  const dpr = window.devicePixelRatio || 1;
  const w = cv.clientWidth; const h = cv.clientHeight;
  cv.width = Math.max(1, Math.round(w * dpr));
  cv.height = Math.max(1, Math.round(h * dpr));
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function draw() {
  const cv = canvas.value; if (!cv || !ctx) return;
  const w = cv.clientWidth; const h = cv.clientHeight; const mid = h / 2;
  ctx.clearRect(0, 0, w, h);
  const s = signal.value; const c = color();
  const cluster = s.cluster; // 0..1 — how tightly channels fold into one line
  const noise = s.ruling === "DEADLOCK" ? 0.5 : (1 - cluster) * 0.25;
  ctx.lineWidth = 1.5;
  ctx.shadowBlur = 12; ctx.shadowColor = c.glow;

  const chans = s.channels.length ? s.channels : [{ amp: 0.2, streaming: false, inGroup: true, idx: 0 }];
  for (const ch of chans) {
    ctx.beginPath();
    // strokeStyle takes a single concrete color (no color-mix in canvas);
    // dim out-group channels via globalAlpha instead.
    ctx.strokeStyle = c.line;
    ctx.globalAlpha = ch.inGroup ? 0.95 : 0.4;
    // Grouped channels share phase; outliers offset. Streaming = jitter.
    const chanPhase = ch.inGroup ? 0 : (ch.idx + 1) * 1.3;
    const jitter = ch.streaming ? 0.18 : 0;
    const amp = ch.amp * mid * 0.7;
    const freq = 0.018 + ch.idx * 0.002;
    for (let x = 0; x <= w; x += 2) {
      const base = Math.sin(x * freq + phase + chanPhase);
      const n = (Math.sin(x * 0.13 + phase * 2.1 + ch.idx) * (noise + jitter));
      const y = mid - (base + n) * amp * (0.6 + s.mean * 0.4);
      x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
  }
  ctx.globalAlpha = 1; ctx.shadowBlur = 0;
}

function frame() {
  phase += 0.05;
  draw();
  raf = requestAnimationFrame(frame);
}

function start() {
  cancelAnimationFrame(raf);
  if (store.instantTokens) { draw(); return; } // static single frame when reduced
  raf = requestAnimationFrame(frame);
}

onMounted(() => {
  ctx = canvas.value?.getContext("2d") ?? null;
  resize();
  ro = new ResizeObserver(() => { resize(); if (store.instantTokens) draw(); });
  if (canvas.value) ro.observe(canvas.value);
  start();
});
watch(() => store.instantTokens, start);
onBeforeUnmount(() => { cancelAnimationFrame(raf); ro?.disconnect(); });
</script>

<template>
  <section class="terminal-panel relative overflow-hidden" aria-label="Convergence oscilloscope">
    <div class="flex items-center justify-between px-3 pt-2 font-mono text-[9px] uppercase tracking-[0.14em] text-fg-faint">
      <span class="glow-text text-signal">◊ signal sync</span>
      <span v-if="signal.ruling === 'CONSENSUS'" class="glow-text text-consensus">▲ signal lock</span>
      <span v-else-if="signal.ruling === 'DEADLOCK'" class="glow-text text-deadlock glitch">✖ desync</span>
      <span v-else class="text-fg-faint">scanning…</span>
    </div>
    <canvas ref="canvas" class="block h-16 w-full" />
    <ConvergenceStrip class="px-3 pb-2" />
  </section>
</template>
