// src/screens/Settings.tsx — langue, thème, journaux, bienvenue.

import { useState } from "react";
import { toApiError } from "../lib/api";
import type { ApiError, AppSettings } from "../lib/types";
import { ErrorNotice, Page } from "../components/Ui";

export function Settings({
  settings, update,
}: {
  settings: AppSettings | null;
  update: (s: AppSettings) => Promise<AppSettings>;
}) {
  const [error, setError] = useState<ApiError | null>(null);
  if (!settings) return <Page title="Paramètres"><p className="muted">Chargement…</p></Page>;

  const apply = async (next: AppSettings) => {
    setError(null);
    try { await update(next); } catch (e) { setError(toApiError(e)); }
  };

  return (
    <Page title="Paramètres" subtitle="Tout reste sur votre PC : aucun compte, aucune télémétrie.">
      {error && <ErrorNotice error={error} onClose={() => setError(null)} />}

      <section className="card flex flex-col gap-4" aria-label="Préférences">
        <label className="flex flex-col gap-1">
          Langue
          <select value={settings.language} disabled aria-describedby="lang-help">
            <option value="fr">Français</option>
          </select>
          <span id="lang-help" className="muted text-xs">Les textes sont externalisés ; l'anglais pourra être ajouté plus tard.</span>
        </label>

        <label className="flex flex-col gap-1">
          Thème
          <select value={settings.theme} onChange={(e) => void apply({ ...settings, theme: e.target.value as AppSettings["theme"] })}>
            <option value="system">Suivre le système</option>
            <option value="light">Clair</option>
            <option value="dark">Sombre</option>
          </select>
        </label>

        <label className="flex flex-col gap-1">
          Conservation des journaux (jours) : <strong>{settings.logRetentionDays}</strong>
          <input
            type="range" min={1} max={90} value={settings.logRetentionDays}
            onChange={(e) => void apply({ ...settings, logRetentionDays: Number(e.target.value) })}
          />
          <span className="muted text-xs">Pris en compte au prochain démarrage de PadBridge.</span>
        </label>
      </section>

      <section className="card flex flex-col gap-3" aria-label="Données locales">
        <h2>Données locales</h2>
        <p className="m-0">
          Profils, réglages et journaux sont stockés dans le dossier de données de l'application sur ce PC. Les journaux ne contiennent
          ni numéros de série, ni chemins personnels, ni contenu de jeu. Rien n'est envoyé sur Internet.
        </p>
        <div>
          <button className="btn" onClick={() => void apply({ ...settings, onboardingDone: false })}>Réafficher l'écran de bienvenue</button>
        </div>
      </section>
    </Page>
  );
}
