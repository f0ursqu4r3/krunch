// Typed wrappers over Tauri commands + the fenced event stream.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { EngineEvent, PresetRow, SessionConfig, SessionDto, StartDto } from "./types";

const EVENT_CHANNEL = "krunch://event";

export const api = {
  coreVersion: () => invoke<string>("core_version"),

  startDeliberation: (idempotencyKey: string, config: SessionConfig, setupJson: string) =>
    invoke<StartDto>("start_deliberation", { idempotencyKey, config, setupJson }),

  answerQuestions: (sessionId: string, answers: [string, string][]) =>
    invoke<boolean>("answer_questions", { sessionId, answers }),

  abandon: (sessionId: string) => invoke<void>("abandon", { sessionId }),

  listSessions: () => invoke<SessionDto[]>("list_sessions"),

  getSession: (sessionId: string) => invoke<SessionDto>("get_session", { sessionId }),

  setCredential: (credentialRef: string, secret: string) =>
    invoke<void>("set_credential", { credentialRef, secret }),

  hasCredential: (credentialRef: string) =>
    invoke<boolean>("has_credential", { credentialRef }),

  exportSession: (sessionId: string) => invoke<string>("export_session", { sessionId }),

  /** Write the dump to ~/Downloads natively and reveal it; returns the path. */
  saveSessionDump: (sessionId: string) => invoke<string>("save_session_dump", { sessionId }),

  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),

  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),

  listPresets: () => invoke<PresetRow[]>("list_presets"),

  savePreset: (name: string, configJson: string) =>
    invoke<string>("save_preset", { name, configJson }),

  deletePreset: (id: string) => invoke<void>("delete_preset", { id }),

  getSessionSetup: (sessionId: string) =>
    invoke<string | null>("get_session_setup", { sessionId }),
};

/** Whether we're running inside the Tauri shell (vs. a plain-browser preview). */
export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** Subscribe to engine events. Returns an unlisten function. */
export function onEngineEvent(handler: (e: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>(EVENT_CHANNEL, (msg) => handler(msg.payload));
}
