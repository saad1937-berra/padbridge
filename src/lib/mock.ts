// src/lib/mock.ts — moteur simulé pour développer l'interface dans un navigateur.
// Reflète le comportement par défaut du vrai moteur : pas de backend XInput disponible.

import type {
  ActivateResult, AppSettings, ControllerView, DiagnosticsReport, InputSample, OutputMode,
  Profile, PsButton, ValidationReport, XButton,
} from "./types";
import { STANDARD_PRESET } from "./types";
import type { Api } from "./api";

const DEVICE_ID = "0123456789abcdef";
const DEFAULT_ID = "profile-default";

function defaultProfile(): Profile {
  return {
    schemaVersion: 1, id: DEFAULT_ID, name: "Standard", outputMode: "native", controllerModel: null,
    deadzones: { leftStick: 0.08, rightStick: 0.08, leftTrigger: 0.02, rightTrigger: 0.02 },
    mapping: {}, rumbleEnabled: false, updatedAt: Date.now(),
  };
}

const profiles = new Map<string, Profile>([[DEFAULT_ID, defaultProfile()]]);
let settings: AppSettings = {
  language: "fr", theme: "system", logRetentionDays: 7, onboardingDone: false, activeProfileId: DEFAULT_ID,
};
let connected = true;
let mode: OutputMode = "native";
let testDevice: string | null = null;
let preview: Profile | null = null;
const listeners = { sample: new Set<(s: InputSample) => void>(), changed: new Set<() => void>() };
const events = [{ timestampMs: Date.now(), severity: "info" as const, errorCode: "PB-I002", module: "devices", redactedMessage: "manette détectée (simulation)" }];

function view(): ControllerView[] {
  if (!connected) return [];
  return [{
    device: {
      deviceId: DEVICE_ID, model: "dualsense", transport: "usb", vendorId: 0x054c, productId: 0x0ce6,
      capabilityFlags: { touchpad: true, gyroscope: true, muteButton: true, adaptiveTriggers: true, lightbar: true },
      lastSeenMs: Date.now(),
    },
    state: { deviceId: DEVICE_ID, connectionState: "connected", battery: { percent: 80, charging: true }, lastInputAtMs: Date.now() },
    mode, virtualSessionId: null, profileId: settings.activeProfileId,
  }];
}

function validate(p: Profile): ValidationReport {
  const errors: ValidationReport["errors"] = [];
  const warnings: ValidationReport["warnings"] = [];
  if (!p.name.trim()) errors.push({ field: "name", code: "NAME_EMPTY", detail: null });
  for (const [k, v] of Object.entries(p.deadzones)) {
    if (!(v >= 0 && v <= 0.5)) errors.push({ field: `deadzones.${k}`, code: "DEADZONE_OUT_OF_RANGE", detail: "0 – 0.5" });
  }
  const eff = { ...STANDARD_PRESET, ...p.mapping };
  const targets = Object.values(eff);
  for (const t of ["A", "B", "Start"] as XButton[]) {
    if (!targets.includes(t)) warnings.push({ field: "mapping", code: "ACTION_UNREACHABLE", detail: t });
  }
  return { errors, warnings };
}

function radial(x: number, y: number, dz: number): [number, number] {
  const m = Math.hypot(x, y);
  if (m <= dz || m === 0) return [0, 0];
  const s = Math.min(1, (Math.min(m, 1) - dz) / (1 - dz));
  return [(x / m) * s, (y / m) * s];
}
const trig = (v: number, t: number) => (v <= t ? 0 : Math.min(1, (v - t) / (1 - t)));

function makeSample(seq: number): InputSample {
  const p = preview ?? profiles.get(settings.activeProfileId) ?? defaultProfile();
  const t = seq / 30;
  const lx = Math.cos(t) * 0.9, ly = Math.sin(t) * 0.9;
  const rx = 0.06 * Math.cos(t * 3), ry = 0.04 * Math.sin(t * 2);
  const pressed: PsButton[] = [];
  if (Math.floor(t) % 4 === 0) pressed.push("cross");
  if (Math.floor(t) % 4 === 1) pressed.push("l1", "dpad_up");
  const l2 = (Math.sin(t / 2) + 1) / 2;
  const eff = { ...STANDARD_PRESET, ...p.mapping };
  const [olx, oly] = radial(lx, ly, p.deadzones.leftStick);
  const [orx, ory] = radial(rx, ry, p.deadzones.rightStick);
  return {
    deviceId: DEVICE_ID, seq, timestampUs: seq * 33000, pressed,
    leftRaw: { x: lx, y: ly }, rightRaw: { x: rx, y: ry }, l2Raw: l2, r2Raw: 0.01,
    output: {
      buttons: pressed.map((b) => eff[b]).filter((b): b is XButton => !!b),
      lx: olx, ly: oly, rx: orx, ry: ory,
      lt: trig(l2, p.deadzones.leftTrigger), rt: trig(0.01, p.deadzones.rightTrigger),
    },
  };
}

let seq = 0;
setInterval(() => {
  if (!connected || !testDevice) return;
  seq += 1;
  const s = makeSample(seq);
  listeners.sample.forEach((cb) => cb(s));
}, 33);

const notify = () => listeners.changed.forEach((cb) => cb());
const mockSystem = {
  os: "windows", arch: "x86_64", osVersion: "10.0.22631.4602 (simulation)", windowsBuild: 22631,
  isWindows11: true, bluetoothPresent: true, elevated: false, steamRunning: false,
};

function report(): DiagnosticsReport {
  return {
    system: mockSystem,
    backend: { available: false, name: "none", code: "PB-E101", reason: "Aucun pilote de manette virtuelle validé n'est installé." },
    controllers: view(),
    doubleInput: { risk: "none", causes: [], guidance: [] },
    events: [...events],
  };
}

export const mockApi: Api = {
  listControllers: async () => view(),
  getControllerState: async (id) => {
    const v = view().find((c) => c.device.deviceId === id);
    if (!v) throw { code: "PB-E201", detail: "appareil inconnu" };
    return v;
  },
  startInputTest: async (id, p) => { testDevice = id; preview = p ?? null; },
  stopInputTest: async () => { testDevice = null; preview = null; },
  testDs4Rumble: async () => { throw { code: "PB-E101", detail: "Vibration HID indisponible dans le navigateur simulé." }; },
  stopDs4Rumble: async () => {},
  inspectDs4Hid: async () => { throw { code: "PB-E101", detail: "Inspection HID uniquement disponible dans PadBridge Tauri sous Windows." }; },
  analyzeStickDrift: async () => { throw { code: "PB-E002", detail: "Le diagnostic de drift réel nécessite PadBridge Tauri et une manette physique." }; },
  validateProfile: async (p) => validate(p),
  saveProfile: async (p) => {
    const report = validate(p);
    if (report.errors.length) throw { code: "PB-E301", detail: `${report.errors[0].field}: ${report.errors[0].code}` };
    const profile = { ...p, name: p.name.trim(), updatedAt: Date.now() };
    profiles.set(profile.id, profile);
    notify();
    return { profile, report };
  },
  listProfiles: async () => [...profiles.values()].sort((a, b) => (a.id === DEFAULT_ID ? -1 : b.id === DEFAULT_ID ? 1 : a.name.localeCompare(b.name))),
  getProfile: async (id) => {
    const p = profiles.get(id);
    if (!p) throw { code: "PB-E302", detail: id };
    return p;
  },
  duplicateProfile: async (id) => {
    const src = profiles.get(id);
    if (!src) throw { code: "PB-E302", detail: id };
    let n = 1, newId = "";
    do { newId = `copie-${n++}`; } while (profiles.has(newId));
    const copy = { ...src, id: newId, name: `${src.name} (copie)`, updatedAt: Date.now() };
    profiles.set(newId, copy);
    return copy;
  },
  renameProfile: async (id, name) => {
    const p = profiles.get(id);
    if (!p) throw { code: "PB-E302", detail: id };
    if (!name.trim()) throw { code: "PB-E301", detail: "name: NAME_EMPTY" };
    const next = { ...p, name: name.trim(), updatedAt: Date.now() };
    profiles.set(id, next);
    return next;
  },
  resetProfile: async (id) => {
    const p = profiles.get(id);
    if (!p) throw { code: "PB-E302", detail: id };
    const next = { ...defaultProfile(), id, name: p.name };
    profiles.set(id, next);
    return next;
  },
  activateMode: async (_id, requested, profileId): Promise<ActivateResult> => {
    if (profileId) settings = { ...settings, activeProfileId: profileId };
    const none = { risk: "none" as const, causes: [], guidance: [] };
    if (requested === "xinput") {
      return { ok: false, requested, mode: "native", sessionId: null, errorCode: "PB-E101", warnings: [], doubleInput: none };
    }
    mode = requested;
    notify();
    return { ok: true, requested, mode, sessionId: null, errorCode: null, warnings: [], doubleInput: none };
  },
  stopVirtualSession: async () => { mode = "native"; notify(); },
  runDiagnostics: async () => report(),
  previewDiagnosticsExport: async () =>
    "PadBridge — diagnostic (export expurgé)\n(simulation navigateur : aperçu indicatif)\n\nSystème\n  Windows : 10.0.22631.4602 (build 22631)\n",
  exportDiagnostics: async () => { events.push({ timestampMs: Date.now(), severity: "info", errorCode: "PB-I030", module: "diagnostics", redactedMessage: "diagnostic exporté (simulation)" }); },
  getSettings: async () => settings,
  saveSettings: async (s) => { settings = s; return s; },
};

export function mockOnInputSample(cb: (s: InputSample) => void) {
  listeners.sample.add(cb);
  return () => { listeners.sample.delete(cb); };
}
export function mockOnControllersChanged(cb: () => void) {
  listeners.changed.add(cb);
  return () => { listeners.changed.delete(cb); };
}
export async function mockPickExportPath(): Promise<string | null> {
  return "C:\\simulation\\padbridge-diagnostic.txt";
}

// Poignée de débogage : window.__padbridgeMock.toggle() simule un débranchement.
if (typeof window !== "undefined") {
  (window as unknown as { __padbridgeMock: unknown }).__padbridgeMock = {
    toggle: () => { connected = !connected; notify(); },
  };
}
