// src/screens/Settings.tsx — langue, thème, journaux, bienvenue.

import { useState } from "react";
import { toApiError } from "../lib/api";
import type { ApiError, AppSettings } from "../lib/types";
import { ErrorNotice, Page } from "../components/Ui";
import { PADBRIDGE_BRAND } from "../lib/branding";

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

      <section className="card flex flex-col gap-3" aria-label="Zone de notification Windows">
        <h2>Zone de notification Windows</h2>
        <p className="m-0">
          Sur Windows, faites un clic droit sur l'icône PadBridge près de l'horloge pour afficher
          « Masquer », « Afficher » ou « Quitter ». Un clic gauche sur l'icône rouvre la fenêtre.
        </p>
        <p className="muted m-0 text-xs">
          Fermer la fenêtre avec × quitte réellement l'application. Si vous la masquez volontairement,
          PadBridge reste actif et continue de lire les entrées HID. Choisissez « Quitter PadBridge »
          dans la zone de notification pour libérer la manette avant de lancer un jeu.
        </p>
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

      <section className="card flex flex-col gap-2" aria-label="À propos de PadBridge">
        <h2>À propos de PadBridge</h2>
        <p className="m-0 font-semibold">{PADBRIDGE_BRAND.byline}</p>
        <p className="muted m-0 text-sm">{PADBRIDGE_BRAND.projectStatus}</p>
        <p className="muted m-0 text-xs">{PADBRIDGE_BRAND.independence}</p>
        <p className="muted m-0 text-xs">Auteur sur GitHub : @{PADBRIDGE_BRAND.creatorGitHub}</p>
      </section>
    </Page>
  );
}
