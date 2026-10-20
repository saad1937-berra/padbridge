// src/screens/TestScreen.tsx — test des touches/axes et calibration avec aperçu en direct.

import { useEffect, useMemo, useState } from "react";
import { api, toApiError } from "../lib/api";
import { modelLabel, transportLabel, xButtonLabel } from "../lib/i18n";
import { useInputSample } from "../lib/useEngine";
import type { ApiError, AppSettings, ControllerView, Profile, ValidationReport } from "../lib/types";
import { DEFAULT_DEADZONES, DeadzoneEditor, ValidationMessages } from "../components/ProfileEditors";
import { Gamepad } from "../components/Gamepad";
import { DriftDiagnostic } from "../components/DriftDiagnostic";
import { RumbleTest } from "../components/RumbleTest";
import { ErrorNotice, Page } from "../components/Ui";

const pct = (v: number) => `${Math.round(Math.max(0, Math.min(1, v)) * 100)}%`;
const signed = (v: number) => (v >= 0 ? "+" : "") + v.toFixed(2);

function Meter({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <div className="flex justify-between"><span>{label}</span><strong>{value.toFixed(2)}</strong></div>
      <div className="bar" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={1} aria-valuenow={Number(value.toFixed(2))}>
        <span style={{ width: pct(value) }} />
      </div>
    </div>
  );
}

export function TestScreen({
  controllers, selectedId, selectDevice, settings,
}: {
  controllers: ControllerView[];
  selectedId: string | null;
  selectDevice: (id: string) => void;
  settings: AppSettings | null;
}) {
  const usable = controllers.filter((c) => c.state.connectionState !== "disconnected");
  const current = usable.find((c) => c.device.deviceId === selectedId) ?? usable[0] ?? null;
  const deviceId = current?.device.deviceId ?? null;

  const [saved, setSaved] = useState<Profile | null>(null);
  const [draft, setDraft] = useState<Profile | null>(null);
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const profileId = settings?.activeProfileId ?? "profile-default";
  useEffect(() => {
    let alive = true;
    void api.getProfile(profileId).then((p) => {
      if (!alive) return;
      setSaved(p);
      setDraft(p);
    }).catch((e) => alive && setError(toApiError(e)));
    return () => { alive = false; };
  }, [profileId]);

  const dirty = useMemo(() => !!draft && !!saved && JSON.stringify(draft) !== JSON.stringify(saved), [draft, saved]);
  const sample = useInputSample(deviceId, dirty ? draft : null);

  useEffect(() => {
    if (!draft) return;
    let alive = true;
    void api.validateProfile(draft).then((r) => alive && setReport(r)).catch(() => undefined);
    return () => { alive = false; };
  }, [draft]);

  const save = async () => {
    if (!draft) return;
    setError(null);
    try {
      const out = await api.saveProfile(draft);
      setSaved(out.profile);
      setDraft(out.profile);
      setMessage("Profil enregistré. L’aperçu utilise ces réglages ; en mode natif, ils ne modifient pas les commandes reçues par le jeu.");
    } catch (e) {
      setError(toApiError(e));
    }
  };

  if (!current) {
    return (
      <Page title="Test de la manette">
        <p className="muted">Aucune manette utilisable. Branchez-en une ou appairez-la dans Windows, puis revenez ici.</p>
      </Page>
    );
  }

  const out = sample?.output;
  const virtualActive = current.mode === "xinput" && current.virtualSessionId !== null;
  return (
    <Page title="Test de la manette" subtitle="Appuyez sur les touches et bougez les sticks : chaque contrôle doit réagir une seule fois.">
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2">
          Manette
          <select value={deviceId ?? ""} onChange={(e) => selectDevice(e.target.value)}>
            {usable.map((c) => (
              <option key={c.device.deviceId} value={c.device.deviceId}>
                {modelLabel[c.device.model]} — {transportLabel[c.device.transport]}
              </option>
            ))}
          </select>
        </label>
        {!sample && <span className="muted">En attente d'une première entrée de la manette…</span>}
      </div>

      {!virtualActive && (
        <div className="notice notice-warn" role="status">
          <strong>Mode natif : aperçu seulement.</strong> PadBridge lit la manette et calcule le mapping et les zones mortes, mais ne modifie pas les entrées du jeu. Aucune manette virtuelle n’est active.
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <section className="card" aria-label="Schéma">
          <h2>Entrées de la manette</h2>
          <Gamepad sample={sample} />
        </section>

        <section className="card flex flex-col gap-3" aria-label="Axes">
          <h2>Axes et gâchettes</h2>
          <p className="muted m-0 text-xs">Valeur brute → valeur calculée après application du profil{dirty ? " (aperçu non enregistré)" : ""}.</p>
          <div className="grid grid-cols-2 gap-3">
            <Meter label="L2 brut" value={sample?.l2Raw ?? 0} />
            <Meter label="L2 calculé" value={out?.lt ?? 0} />
            <Meter label="R2 brut" value={sample?.r2Raw ?? 0} />
            <Meter label="R2 calculé" value={out?.rt ?? 0} />
          </div>
          <table className="data">
            <thead><tr><th>Stick</th><th>X brut</th><th>Y brut</th><th>X calculé</th><th>Y calculé</th></tr></thead>
            <tbody>
              <tr>
                <th scope="row">Gauche</th>
                <td>{signed(sample?.leftRaw.x ?? 0)}</td><td>{signed(sample?.leftRaw.y ?? 0)}</td>
                <td>{signed(out?.lx ?? 0)}</td><td>{signed(out?.ly ?? 0)}</td>
              </tr>
              <tr>
                <th scope="row">Droit</th>
                <td>{signed(sample?.rightRaw.x ?? 0)}</td><td>{signed(sample?.rightRaw.y ?? 0)}</td>
                <td>{signed(out?.rx ?? 0)}</td><td>{signed(out?.ry ?? 0)}</td>
              </tr>
            </tbody>
          </table>
          <p className="m-0">
            <strong>Actions calculées :</strong>{" "}
            {out?.buttons.length ? out.buttons.map((b) => xButtonLabel[b]).join(", ") : <span className="muted">aucune</span>}
          </p>
        </section>
      </div>

      <DriftDiagnostic deviceId={current.device.deviceId} sample={sample} />
      <RumbleTest device={current.device} connected={current.state.connectionState === "connected" || current.state.connectionState === "unstable"} />

      {draft && (
        <section className="card flex flex-col gap-3" aria-label="Calibration">
          <h2>Calibration logicielle</h2>
          <DeadzoneEditor profile={draft} onChange={(p) => { setDraft(p); setMessage(null); }} />
          <ValidationMessages report={report} />
          {error && <ErrorNotice error={error} onClose={() => setError(null)} />}
          {message && <div className="notice notice-ok" role="status">{message}</div>}
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" onClick={() => void save()} disabled={!dirty || !!report?.errors.length}>
              Enregistrer
            </button>
            <button className="btn" onClick={() => setDraft({ ...draft, deadzones: { ...DEFAULT_DEADZONES } })}>
              Remettre les valeurs initiales
            </button>
            <button className="btn" onClick={() => saved && setDraft(saved)} disabled={!dirty}>Annuler les changements</button>
          </div>
          <p className="muted m-0 text-xs">Profil modifié : {draft.name}. Les réglages s'appliquent aussi aux manettes déjà connectées qui l'utilisent.</p>
        </section>
      )}
    </Page>
  );
}
