// src/screens/Profiles.tsx — profils : liste, duplication, renommage, réinitialisation, mapping.

import { useEffect, useMemo, useState } from "react";
import { api, toApiError } from "../lib/api";
import type { ApiError, AppSettings, ControllerView, Profile, ValidationReport } from "../lib/types";
import { DeadzoneEditor, MappingEditor, ValidationMessages } from "../components/ProfileEditors";
import { ErrorNotice, Page } from "../components/Ui";

interface Props {
  profiles: Profile[];
  refreshProfiles: () => Promise<void>;
  controllers: ControllerView[];
  settings: AppSettings | null;
  updateSettings: (s: AppSettings) => Promise<AppSettings>;
  refreshControllers: () => Promise<void>;
}

export function Profiles({ profiles, refreshProfiles, controllers, settings, updateSettings, refreshControllers }: Props) {
  const [editingId, setEditingId] = useState<string>(settings?.activeProfileId ?? "profile-default");
  const stored = profiles.find((p) => p.id === editingId) ?? profiles[0] ?? null;
  const [draft, setDraft] = useState<Profile | null>(stored);
  const [name, setName] = useState(stored?.name ?? "");
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    setDraft(stored);
    setName(stored?.name ?? "");
    setMessage(null);
    // On repart de la version enregistrée quand on change de profil ou qu'elle est rechargée.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stored?.id, stored?.updatedAt]);

  useEffect(() => {
    if (!draft) return;
    let alive = true;
    void api.validateProfile(draft).then((r) => alive && setReport(r)).catch(() => undefined);
    return () => { alive = false; };
  }, [draft]);

  const dirty = useMemo(
    () => !!draft && !!stored && (JSON.stringify(draft) !== JSON.stringify(stored) || name.trim() !== stored.name),
    [draft, stored, name],
  );

  const run = async (fn: () => Promise<string | void>) => {
    setError(null);
    setMessage(null);
    try {
      const msg = await fn();
      if (msg) setMessage(msg);
    } catch (e) {
      setError(toApiError(e));
    }
  };

  const save = () =>
    run(async () => {
      if (!draft) return;
      await api.saveProfile({ ...draft, name });
      await refreshProfiles();
      await refreshControllers();
      return "Profil enregistré.";
    });

  const duplicate = () =>
    run(async () => {
      if (!stored) return;
      const copy = await api.duplicateProfile(stored.id);
      await refreshProfiles();
      setEditingId(copy.id);
      return "Copie créée.";
    });

  const reset = () =>
    run(async () => {
      if (!stored) return;
      await api.resetProfile(stored.id);
      await refreshProfiles();
      await refreshControllers();
      return "Profil remis aux valeurs initiales.";
    });

  const useProfile = () =>
    run(async () => {
      if (!stored || !settings) return;
      await updateSettings({ ...settings, activeProfileId: stored.id });
      for (const c of controllers) {
        if (c.state.connectionState !== "disconnected") {
          await api.activateMode(c.device.deviceId, c.mode, stored.id);
        }
      }
      await refreshControllers();
      return `« ${stored.name} » est maintenant le profil actif.`;
    });

  const isActive = stored?.id === settings?.activeProfileId;

  return (
    <Page title="Profils" subtitle="Un profil regroupe le mapping des touches et les zones mortes. Le profil actif s'applique aux manettes connectées.">
      <div className="grid gap-4 md:grid-cols-[260px_1fr]">
        <nav className="card" aria-label="Liste des profils">
          <ul className="m-0 flex list-none flex-col gap-1 p-0">
            {profiles.map((p) => (
              <li key={p.id}>
                <button
                  className="nav-item"
                  aria-current={p.id === stored?.id ? "true" : undefined}
                  aria-controls="profile-editor"
                  onClick={() => setEditingId(p.id)}
                >
                  <span>{p.name}</span>
                  {p.id === settings?.activeProfileId && <span className="badge badge-ok">actif</span>}
                </button>
              </li>
            ))}
          </ul>
        </nav>

        {draft && stored && (
          <div id="profile-editor" className="flex flex-col gap-4">
            <section className="card flex flex-col gap-3" aria-label="Profil sélectionné">
              <label className="flex flex-col gap-1">
                Nom du profil
                <input type="text" value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
              </label>
              <div className="flex flex-wrap gap-2">
                <button className="btn btn-primary" onClick={() => void save()} disabled={!dirty || !!report?.errors.length || !name.trim()}>
                  Enregistrer
                </button>
                <button className="btn" onClick={() => void useProfile()} disabled={isActive || dirty}>Utiliser ce profil</button>
                <button className="btn" onClick={() => void duplicate()}>Dupliquer</button>
                <button className="btn" onClick={() => void reset()}>Réinitialiser</button>
              </div>
              {dirty && <p className="muted m-0 text-xs">Modifications non enregistrées.</p>}
              <ValidationMessages report={report} />
              {error && <ErrorNotice error={error} onClose={() => setError(null)} />}
              {message && <div className="notice notice-ok" role="status">{message}</div>}
            </section>

            <section className="card" aria-label="Mapping">
              <h2>Mapping des touches</h2>
              <MappingEditor profile={draft} onChange={setDraft} />
            </section>

            <section className="card" aria-label="Zones mortes">
              <h2>Zones mortes</h2>
              <DeadzoneEditor profile={draft} onChange={setDraft} />
            </section>
          </div>
        )}
      </div>
    </Page>
  );
}
