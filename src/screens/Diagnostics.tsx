// src/screens/Diagnostics.tsx — prérequis, événements et export expurgé avec consentement.

import { useCallback, useEffect, useState } from "react";
import { api, pickExportPath, toApiError } from "../lib/api";
import { errorText, formatDate } from "../lib/i18n";
import type { ApiError, DiagnosticsReport } from "../lib/types";
import { ErrorNotice, Page, Yes } from "../components/Ui";

const severityInfo = {
  info: { label: "Info", icon: "ℹ", tone: "neutral" },
  warning: { label: "Avertissement", icon: "▲", tone: "warn" },
  error: { label: "Erreur", icon: "✖", tone: "err" },
} as const;

export function Diagnostics() {
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [consent, setConsent] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setReport(await api.runDiagnostics());
    } catch (e) {
      setError(toApiError(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const showPreview = async () => {
    setError(null);
    setMessage(null);
    try {
      setPreview(await api.previewDiagnosticsExport());
      setConsent(false);
    } catch (e) {
      setError(toApiError(e));
    }
  };

  const doExport = async () => {
    setError(null);
    try {
      const path = await pickExportPath();
      if (!path) return;
      await api.exportDiagnostics(path);
      setMessage("Diagnostic exporté.");
      setPreview(null);
      await load();
    } catch (e) {
      setError(toApiError(e));
    }
  };

  const s = report?.system;
  return (
    <Page title="Diagnostics" subtitle="Prérequis, événements récents et export volontaire d'un rapport expurgé.">
      <div className="flex gap-2">
        <button className="btn" onClick={() => void load()}>Rafraîchir</button>
        <button className="btn btn-primary" onClick={() => void showPreview()}>Préparer un export…</button>
      </div>

      {error && <ErrorNotice error={error} onClose={() => setError(null)} />}
      {message && <div className="notice notice-ok" role="status">{message}</div>}

      {preview !== null && (
        <section className="card flex flex-col gap-3" aria-label="Aperçu de l'export">
          <h2>Contenu qui sera exporté</h2>
          <pre className="m-0 max-h-72 overflow-auto rounded-md p-3 text-xs" style={{ background: "var(--surface-2)" }}>{preview}</pre>
          <p className="muted m-0 text-xs">
            Le rapport ne contient ni numéro de série, ni identifiants matériels, ni chemins personnels, ni contenu de jeu.
          </p>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
            J'ai relu ce contenu et j'autorise son export vers un fichier que je choisis.
          </label>
          <div className="flex gap-2">
            <button className="btn btn-primary" disabled={!consent} onClick={() => void doExport()}>Choisir l'emplacement et exporter</button>
            <button className="btn" onClick={() => setPreview(null)}>Annuler</button>
          </div>
        </section>
      )}

      <section className="card" aria-label="Prérequis">
        <h2>Prérequis</h2>
        {!s ? <p className="muted m-0">Chargement…</p> : (
          <table className="data">
            <tbody>
              <tr><th scope="row">Windows</th><td>{s.osVersion ?? "Inconnu"} ({s.arch})</td></tr>
              <tr><th scope="row">Windows 11</th><td><Yes value={s.isWindows11} /></td></tr>
              <tr><th scope="row">Bluetooth</th><td><Yes value={s.bluetoothPresent} /></td></tr>
              <tr><th scope="row">Droits élevés</th><td><Yes value={s.elevated} /></td></tr>
              <tr><th scope="row">Steam en cours</th><td><Yes value={s.steamRunning} /></td></tr>
              <tr>
                <th scope="row">Pilote virtuel</th>
                <td>
                  {report?.backend.available ? "Disponible" : `Indisponible (${report?.backend.code ?? "?"}) — ${errorText(report?.backend.code ?? "PB-E101").action}`}
                </td>
              </tr>
            </tbody>
          </table>
        )}
      </section>

      <section className="card p-0" aria-label="Événements">
        <table className="data">
          <thead><tr><th>Heure</th><th>Gravité</th><th>Code</th><th>Module</th><th>Message</th></tr></thead>
          <tbody>
            {report?.events.length ? [...report.events].reverse().map((e, i) => {
              const sev = severityInfo[e.severity];
              return (
                <tr key={`${e.timestampMs}-${i}`}>
                  <td>{formatDate(e.timestampMs)}</td>
                  <td><span className={`badge badge-${sev.tone}`}><span aria-hidden="true">{sev.icon}</span>{sev.label}</span></td>
                  <td>{e.errorCode}</td>
                  <td>{e.module}</td>
                  <td>{e.redactedMessage}</td>
                </tr>
              );
            }) : (
              <tr><td colSpan={5} className="muted">Aucun événement.</td></tr>
            )}
          </tbody>
        </table>
      </section>
    </Page>
  );
}
