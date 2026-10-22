// src/components/Ui.tsx — petits composants partagés.

import type { ReactNode } from "react";
import { errorText, modeLabel, stateInfo, transportLabel } from "../lib/i18n";
import type { ApiError, Battery, ConnectionState, OutputMode, Transport } from "../lib/types";

export function Page({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-4">
      <header>
        <h1 id="page-title" tabIndex={-1} className="m-0 text-xl font-semibold">{title}</h1>
        {subtitle && <p className="muted m-0 mt-1">{subtitle}</p>}
      </header>
      {children}
    </section>
  );
}

export function StatusBadge({ state }: { state: ConnectionState }) {
  const { label, icon, tone } = stateInfo[state];
  return (
    <span className={`badge badge-${tone}`}>
      <span aria-hidden="true">{icon}</span>
      {label}
    </span>
  );
}

export function TransportBadge({ transport }: { transport: Transport }) {
  return <span className="badge badge-neutral">{transportLabel[transport]}</span>;
}

export function ModeBadge({ mode, virtualActive }: { mode: OutputMode; virtualActive: boolean }) {
  return (
    <span className={`badge ${virtualActive ? "badge-ok" : "badge-neutral"}`}>
      <span aria-hidden="true">{virtualActive ? "◆" : "◇"}</span>
      {modeLabel[mode]}
      {virtualActive ? " — manette virtuelle active" : ""}
    </span>
  );
}

/** Batterie : « Inconnue » quand la donnée manque, jamais 0 % par défaut (PB-009). */
export function batteryText(battery: Battery | null): string {
  if (!battery || battery.percent === null) return "Inconnue";
  return `≈ ${battery.percent} %${battery.charging ? " (en charge)" : ""}`;
}

export function ErrorNotice({ error, onClose }: { error: ApiError; onClose?: () => void }) {
  const text = errorText(error.code);
  return (
    <div className="notice notice-err" role="alert">
      <div className="flex items-start justify-between gap-3">
        <strong>
          {text.title} <span className="muted font-normal">({error.code})</span>
        </strong>
        {onClose && (
          <button className="btn" onClick={onClose} aria-label="Fermer le message d'erreur">
            Fermer
          </button>
        )}
      </div>
      <p className="m-0 mt-1">{text.explanation}</p>
      <p className="m-0 mt-1">
        <strong>À faire :</strong> {text.action}
      </p>
      {error.detail && <p className="muted m-0 mt-1 text-xs">Détail technique : {error.detail}</p>}
    </div>
  );
}

export function Yes({ value }: { value: boolean | null }) {
  if (value === null) return <span className="muted">Inconnu</span>;
  return <span>{value ? "Oui" : "Non"}</span>;
}
