// src/lib/api.ts — seul point d'accès au moteur Rust (IPC Tauri).
// Hors Tauri (navigateur via `npm run dev`), un moteur simulé (mock.ts) prend le relais.

import type {
  ActivateResult, ApiError, AppSettings, ControllerView, DiagnosticsReport, InputSample,
  OutputMode, Profile, SaveOutcome, ValidationReport, DriftReport, DriftSample, HidOutputInfo,
} from "./types";
import { mockApi, mockOnControllersChanged, mockOnInputSample, mockPickExportPath } from "./mock";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export function toApiError(e: unknown): ApiError {
  if (e && typeof e === "object" && "code" in e) {
    const { code, detail } = e as { code: unknown; detail?: unknown };
    return { code: String(code), detail: String(detail ?? "") };
  }
  return { code: "PB-E002", detail: String(e) };
}

export interface Api {
  listControllers(): Promise<ControllerView[]>;
  getControllerState(deviceId: string): Promise<ControllerView>;
  startInputTest(deviceId: string, profile?: Profile | null): Promise<void>;
  stopInputTest(): Promise<void>;
  testDs4Rumble(deviceId: string, intensity: number, durationMs: number): Promise<void>;
  stopDs4Rumble(deviceId: string): Promise<void>;
  analyzeStickDrift(samples: DriftSample[]): Promise<DriftReport>;
  validateProfile(profile: Profile): Promise<ValidationReport>;
  saveProfile(profile: Profile): Promise<SaveOutcome>;
  listProfiles(): Promise<Profile[]>;
  getProfile(id: string): Promise<Profile>;
  duplicateProfile(id: string): Promise<Profile>;
  renameProfile(id: string, name: string): Promise<Profile>;
  resetProfile(id: string): Promise<Profile>;
  activateMode(deviceId: string, mode: OutputMode, profileId?: string | null): Promise<ActivateResult>;
  stopVirtualSession(sessionId: string): Promise<void>;
  runDiagnostics(): Promise<DiagnosticsReport>;
  previewDiagnosticsExport(): Promise<string>;
  exportDiagnostics(path: string): Promise<void>;
  getSettings(): Promise<AppSettings>;
  saveSettings(settings: AppSettings): Promise<AppSettings>;
}

const tauriApi: Api = {
  listControllers: () => call("list_controllers"),
  getControllerState: (deviceId) => call("get_controller_state", { deviceId }),
  startInputTest: (deviceId, profile) => call("start_input_test", { deviceId, profile: profile ?? null }),
  stopInputTest: () => call("stop_input_test"),
  testDs4Rumble: (deviceId, intensity, durationMs) => call("test_ds4_rumble", { deviceId, intensity, durationMs }),
  stopDs4Rumble: (deviceId) => call("stop_ds4_rumble", { deviceId }),
  analyzeStickDrift: (samples) => call("analyze_stick_drift", { samples }),
  validateProfile: (profile) => call("validate_profile", { profile }),
  saveProfile: (profile) => call("save_profile", { profile }),
  listProfiles: () => call("list_profiles"),
  getProfile: (id) => call("get_profile", { id }),
  duplicateProfile: (id) => call("duplicate_profile", { id }),
  renameProfile: (id, name) => call("rename_profile", { id, name }),
  resetProfile: (id) => call("reset_profile", { id }),
  activateMode: (deviceId, mode, profileId) =>
    call("activate_mode", { deviceId, mode, profileId: profileId ?? null }),
  stopVirtualSession: (sessionId) => call("stop_virtual_session", { sessionId }),
  runDiagnostics: () => call("run_diagnostics"),
  previewDiagnosticsExport: () => call("preview_diagnostics_export"),
  exportDiagnostics: (path) => call("export_diagnostics", { path }),
  getSettings: () => call("get_settings"),
  saveSettings: (settings) => call("save_settings", { settings }),
};

export const api: Api = inTauri ? tauriApi : mockApi;

type Unlisten = () => void;

export async function onInputSample(cb: (s: InputSample) => void): Promise<Unlisten> {
  if (!inTauri) return mockOnInputSample(cb);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<InputSample>("input-sample", (e) => cb(e.payload));
}

export async function onControllersChanged(cb: () => void): Promise<Unlisten> {
  if (!inTauri) return mockOnControllersChanged(cb);
  const { listen } = await import("@tauri-apps/api/event");
  return listen("controllers-changed", () => cb());
}

/** Boîte de dialogue « Enregistrer sous » ; `null` si l'utilisateur annule. */
export async function pickExportPath(): Promise<string | null> {
  if (!inTauri) return mockPickExportPath();
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({
    title: "Exporter le diagnostic",
    defaultPath: "padbridge-diagnostic.txt",
    filters: [{ name: "Texte", extensions: ["txt"] }],
  });
}
