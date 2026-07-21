import { defineStore } from "pinia";
import { ref } from "vue";
import { api, isTauri } from "@/lib/api";
import type { PresetRow, SetupSnapshot } from "@/lib/types";

const LAST_SETUP_KEY = "last_setup";
const EFFECTS_KEY = "effects";
const EFFECTS_MIRROR = "krunch-effects"; // synchronous pre-paint hint

export type EffectsLevel = "off" | "ambient" | "max";

export const useSettings = defineStore("settings", () => {
  const presets = ref<PresetRow[]>([]);

  async function loadPresets() {
    if (!isTauri()) return;
    try { presets.value = await api.listPresets(); } catch { /* preview */ }
  }
  async function savePreset(name: string, snap: SetupSnapshot) {
    if (!isTauri()) return;
    try { await api.savePreset(name, JSON.stringify(snap)); await loadPresets(); } catch { /* preview */ }
  }
  async function removePreset(id: string) {
    if (!isTauri()) return;
    try { await api.deletePreset(id); await loadPresets(); } catch { /* preview */ }
  }

  async function saveLastSetup(snap: SetupSnapshot) {
    if (!isTauri()) return;
    try { await api.setSetting(LAST_SETUP_KEY, JSON.stringify(snap)); } catch { /* preview */ }
  }
  async function loadLastSetup(): Promise<SetupSnapshot | null> {
    if (!isTauri()) return null;
    try {
      const raw = await api.getSetting(LAST_SETUP_KEY);
      return raw ? (JSON.parse(raw) as SetupSnapshot) : null;
    } catch { return null; }
  }

  /** Write effects to both the localStorage mirror (sync) and the DB (source of truth). */
  async function persistEffects(value: EffectsLevel) {
    localStorage.setItem(EFFECTS_MIRROR, value);
    if (!isTauri()) return;
    try { await api.setSetting(EFFECTS_KEY, JSON.stringify(value)); } catch { /* preview */ }
  }
  /** DB wins if present; else import the current (localStorage-seeded) value into the DB once. */
  async function reconcileEffects(current: EffectsLevel): Promise<EffectsLevel> {
    if (!isTauri()) return current;
    try {
      const raw = await api.getSetting(EFFECTS_KEY);
      if (raw) return JSON.parse(raw) as EffectsLevel;
      await api.setSetting(EFFECTS_KEY, JSON.stringify(current));
      return current;
    } catch { return current; }
  }

  return {
    presets, loadPresets, savePreset, removePreset,
    saveLastSetup, loadLastSetup, persistEffects, reconcileEffects,
  };
});
