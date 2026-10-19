// src/lib/types.ts — miroir des structures JSON du moteur Rust (camelCase).

export type PsButton =
  | "cross" | "circle" | "square" | "triangle"
  | "l1" | "r1" | "l3" | "r3"
  | "share" | "options" | "ps" | "touchpad" | "mute"
  | "dpad_up" | "dpad_down" | "dpad_left" | "dpad_right";

export type XButton =
  | "A" | "B" | "X" | "Y" | "LB" | "RB" | "Back" | "Start" | "Guide" | "LS" | "RS"
  | "DpadUp" | "DpadDown" | "DpadLeft" | "DpadRight";

export const PS_BUTTONS: PsButton[] = [
  "cross", "circle", "square", "triangle", "l1", "r1", "l3", "r3",
  "share", "options", "ps", "touchpad", "mute",
  "dpad_up", "dpad_down", "dpad_left", "dpad_right",
];

export const X_BUTTONS: XButton[] = [
  "A", "B", "X", "Y", "LB", "RB", "Back", "Start", "Guide", "LS", "RS",
  "DpadUp", "DpadDown", "DpadLeft", "DpadRight",
];

/** Préréglage standard — identique à `standard_preset()` côté Rust. */
export const STANDARD_PRESET: Partial<Record<PsButton, XButton>> = {
  cross: "A", circle: "B", square: "X", triangle: "Y",
  l1: "LB", r1: "RB", share: "Back", options: "Start", ps: "Guide",
  l3: "LS", r3: "RS",
  dpad_up: "DpadUp", dpad_down: "DpadDown", dpad_left: "DpadLeft", dpad_right: "DpadRight",
};

export type OutputMode = "native" | "xinput" | "diagnostic";
export type ControllerModel = "dualshock4" | "dualsense" | "dualsenseedge" | "unknown";
export type Transport = "usb" | "bluetooth" | "unknown";
export type ConnectionState = "connected" | "unstable" | "unreadable" | "disconnected";

export interface CapabilityFlags {
  touchpad: boolean;
  gyroscope: boolean;
  muteButton: boolean;
  adaptiveTriggers: boolean;
  lightbar: boolean;
}

export interface ControllerDevice {
  deviceId: string;
  model: ControllerModel;
  transport: Transport;
  vendorId: number;
  productId: number;
  capabilityFlags: CapabilityFlags;
  lastSeenMs: number;
}

export interface Battery {
  percent: number | null;
  charging: boolean;
}

export interface ControllerState {
  deviceId: string;
  connectionState: ConnectionState;
  battery: Battery | null;
  lastInputAtMs: number | null;
}

export interface ControllerView {
  device: ControllerDevice;
  state: ControllerState;
  mode: OutputMode;
  virtualSessionId: string | null;
  profileId: string;
}

export interface Stick { x: number; y: number }

export interface OutputState {
  buttons: XButton[];
  lx: number; ly: number; rx: number; ry: number;
  lt: number; rt: number;
}

export interface InputSample {
  deviceId: string;
  seq: number;
  timestampUs: number;
  pressed: PsButton[];
  leftRaw: Stick;
  rightRaw: Stick;
  l2Raw: number;
  r2Raw: number;
  output: OutputState;
}

export interface Deadzones {
  leftStick: number;
  rightStick: number;
  leftTrigger: number;
  rightTrigger: number;
}

export interface Profile {
  schemaVersion: number;
  id: string;
  name: string;
  outputMode: OutputMode;
  controllerModel?: ControllerModel | null;
  deadzones: Deadzones;
  mapping: Partial<Record<PsButton, XButton>>;
  rumbleEnabled: boolean;
  updatedAt: number;
}

export interface ValidationIssue { field: string; code: string; detail: string | null }
export interface ValidationReport { errors: ValidationIssue[]; warnings: ValidationIssue[] }
export interface SaveOutcome { profile: Profile; report: ValidationReport }

export type Risk = "none" | "possible" | "likely";
export interface DoubleInputAssessment { risk: Risk; causes: string[]; guidance: string[] }

export interface ActivateResult {
  ok: boolean;
  requested: OutputMode;
  mode: OutputMode;
  sessionId: string | null;
  errorCode: string | null;
  warnings: string[];
  doubleInput: DoubleInputAssessment;
}

export interface SystemReport {
  os: string;
  arch: string;
  osVersion: string | null;
  windowsBuild: number | null;
  isWindows11: boolean | null;
  bluetoothPresent: boolean | null;
  elevated: boolean | null;
  steamRunning: boolean;
}

export interface BackendStatus {
  available: boolean;
  name: string;
  code: string | null;
  reason: string | null;
}

export interface DiagnosticEvent {
  timestampMs: number;
  severity: "info" | "warning" | "error";
  errorCode: string;
  module: string;
  redactedMessage: string;
}

export interface DiagnosticsReport {
  system: SystemReport;
  backend: BackendStatus;
  controllers: ControllerView[];
  doubleInput: DoubleInputAssessment;
  events: DiagnosticEvent[];
}

export interface AppSettings {
  language: string;
  theme: "system" | "light" | "dark";
  logRetentionDays: number;
  onboardingDone: boolean;
  activeProfileId: string;
}

export interface ApiError { code: string; detail: string }
