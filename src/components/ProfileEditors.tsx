// src/components/ProfileEditors.tsx — éditeurs de zones mortes et de mapping (profil global).

import { psButtonLabel, xButtonLabel, issueText } from "../lib/i18n";
import {
  PS_BUTTONS, STANDARD_PRESET, X_BUTTONS,
  type Deadzones, type Profile, type PsButton, type ValidationReport, type XButton,
} from "../lib/types";

export const DEFAULT_DEADZONES: Deadzones = {
  leftStick: 0.08, rightStick: 0.08, leftTrigger: 0.02, rightTrigger: 0.02,
};

const DEADZONE_FIELDS: { key: keyof Deadzones; label: string }[] = [
  { key: "leftStick", label: "Zone morte du stick gauche" },
  { key: "rightStick", label: "Zone morte du stick droit" },
  { key: "leftTrigger", label: "Seuil de la gâchette L2" },
  { key: "rightTrigger", label: "Seuil de la gâchette R2" },
];

export function DeadzoneEditor({ profile, onChange }: { profile: Profile; onChange: (p: Profile) => void }) {
  return (
    <div className="flex flex-col gap-3">
      {DEADZONE_FIELDS.map(({ key, label }) => (
        <label key={key} className="flex flex-col gap-1">
          <span className="flex justify-between">
            <span>{label}</span>
            <strong>{profile.deadzones[key].toFixed(2)}</strong>
          </span>
          <input
            type="range" min={0} max={0.5} step={0.01}
            value={profile.deadzones[key]}
            onChange={(e) => onChange({ ...profile, deadzones: { ...profile.deadzones, [key]: Number(e.target.value) } })}
          />
        </label>
      ))}
      <p className="muted m-0 text-xs">
        Limites autorisées : 0 à 0,5. Ces réglages agissent sur la sortie de PadBridge uniquement, pas sur la manette ni son firmware.
      </p>
    </div>
  );
}

function effective(profile: Profile, b: PsButton): XButton | undefined {
  return profile.mapping[b] ?? STANDARD_PRESET[b];
}

/** Ne conserve que les écarts par rapport au préréglage standard. */
function withTarget(profile: Profile, source: PsButton, target: XButton | ""): Profile {
  const mapping = { ...profile.mapping };
  if (target === "" || target === STANDARD_PRESET[source]) delete mapping[source];
  else mapping[source] = target;
  return { ...profile, mapping };
}

export function MappingEditor({ profile, onChange }: { profile: Profile; onChange: (p: Profile) => void }) {
  const swapCrossCircle = () => {
    const cross = effective(profile, "cross");
    const circle = effective(profile, "circle");
    if (!cross || !circle) return;
    onChange(withTarget(withTarget(profile, "cross", circle), "circle", cross));
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <button className="btn" onClick={swapCrossCircle}>Inverser Croix ✕ et Rond ○</button>
        <button className="btn" onClick={() => onChange({ ...profile, mapping: {} })}>Rétablir le préréglage standard</button>
      </div>
      <table className="data">
        <thead>
          <tr><th>Touche de la manette</th><th>Action envoyée au jeu</th></tr>
        </thead>
        <tbody>
          {PS_BUTTONS.map((b) => {
            const hasStandard = STANDARD_PRESET[b] !== undefined;
            return (
              <tr key={b}>
                <td><label htmlFor={`map-${b}`}>{psButtonLabel[b]}</label></td>
                <td>
                  <select
                    id={`map-${b}`}
                    value={effective(profile, b) ?? ""}
                    onChange={(e) => onChange(withTarget(profile, b, e.target.value as XButton | ""))}
                  >
                    {!hasStandard && <option value="">Aucune action</option>}
                    {X_BUTTONS.map((x) => (
                      <option key={x} value={x}>{xButtonLabel[x]}</option>
                    ))}
                  </select>
                  {profile.mapping[b] !== undefined && <span className="muted ml-2 text-xs">modifié</span>}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function ValidationMessages({ report }: { report: ValidationReport | null }) {
  if (!report || (!report.errors.length && !report.warnings.length)) return null;
  return (
    <div className="flex flex-col gap-2">
      {report.errors.map((i, n) => (
        <div key={`e${n}`} className="notice notice-err" role="alert">Erreur — {issueText(i)}</div>
      ))}
      {report.warnings.map((i, n) => (
        <div key={`w${n}`} className="notice notice-warn">Attention — {issueText(i)}</div>
      ))}
    </div>
  );
}
