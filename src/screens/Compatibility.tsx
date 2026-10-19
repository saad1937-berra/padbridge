// src/screens/Compatibility.tsx — pilote, modes, Steam Input, niveaux de prise en charge (PB-004).

import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { causeText, errorText, guidanceText, modeHelp, modeLabel } from "../lib/i18n";
import type { ControllerView, DiagnosticsReport } from "../lib/types";
import { Page } from "../components/Ui";

type Level = "supported" | "validating" | "unsupported";
const levelInfo: Record<Level, { label: string; icon: string; tone: string }> = {
  supported: { label: "Pris en charge", icon: "✔", tone: "ok" },
  validating: { label: "En cours de validation", icon: "◔", tone: "warn" },
  unsupported: { label: "Non pris en charge", icon: "✖", tone: "err" },
};

function Level({ level }: { level: Level }) {
  const { label, icon, tone } = levelInfo[level];
  return <span className={`badge badge-${tone}`}><span aria-hidden="true">{icon}</span>{label}</span>;
}

export function Compatibility({ controllers, goto }: { controllers: ControllerView[]; goto: (s: "test" | "diag") => void }) {
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  useEffect(() => {
    let alive = true;
    void api.runDiagnostics().then((r) => alive && setReport(r)).catch(() => undefined);
    return () => { alive = false; };
  }, [controllers]);

  const backend = report?.backend;
  const xinputLevel: Level = !backend ? "validating" : backend.available ? "validating" : "unsupported";
  const double = report?.doubleInput;

  return (
    <Page title="Compatibilité" subtitle="Ce que PadBridge peut réellement faire sur ce PC, sans promesse de compatibilité universelle.">
      <section className="card" aria-label="Pilote de manette virtuelle">
        <h2>Pilote de manette virtuelle</h2>
        {!backend ? (
          <p className="muted m-0">Vérification…</p>
        ) : backend.available ? (
          <p className="m-0">Un pilote est disponible ({backend.name}). Le mode XInput reste « en cours de validation » tant que les jeux de référence n'ont pas été testés.</p>
        ) : (
          <>
            <div className="notice notice-warn">
              <strong>{errorText(backend.code ?? "PB-E101").title}</strong>
              <p className="m-0 mt-1">{errorText(backend.code ?? "PB-E101").explanation}</p>
            </div>
            <p className="mb-0 mt-3"><strong>Installation volontaire :</strong> PadBridge n'installe jamais de pilote en silence ni ne contourne la signature des pilotes. Tant qu'aucun pilote validé (signé, redistribuable, maintenu) n'est retenu, l'application fonctionne en mode natif et diagnostic. ViGEmBus est officiellement retiré : ne l'installez pas pour PadBridge.</p>
          </>
        )}
      </section>

      <section className="card p-0" aria-label="Modes">
        <table className="data">
          <thead><tr><th>Mode</th><th>Principe</th><th>Niveau sur ce PC</th></tr></thead>
          <tbody>
            <tr>
              <th scope="row">{modeLabel.native}</th><td>{modeHelp.native}</td>
              <td><Level level="supported" /></td>
            </tr>
            <tr>
              <th scope="row">{modeLabel.xinput}</th><td>{modeHelp.xinput}</td>
              <td><Level level={xinputLevel} /></td>
            </tr>
            <tr>
              <th scope="row">Steam Input (externe)</th>
              <td>PadBridge explique la cohabitation mais ne pilote pas Steam. Les réglages précis restent dans Steam.</td>
              <td><Level level="validating" /></td>
            </tr>
            <tr>
              <th scope="row">{modeLabel.diagnostic}</th><td>{modeHelp.diagnostic}</td>
              <td><Level level="supported" /></td>
            </tr>
          </tbody>
        </table>
      </section>

      <section className="card" aria-label="Double entrée">
        <h2>Double entrée et Steam Input</h2>
        {!double ? (
          <p className="muted m-0">Analyse…</p>
        ) : double.risk === "none" ? (
          <p className="m-0">Aucun risque détecté avec la configuration actuelle.{report?.system.steamRunning ? " Steam est ouvert : s'il remappe votre manette, désactivez-le pour ce jeu si vous voyez des doublons." : ""}</p>
        ) : (
          <div className="notice notice-warn">
            <ul className="m-0 pl-5">{double.causes.map((c) => <li key={c}>{causeText[c] ?? c}</li>)}</ul>
            <p className="m-0 mt-2"><strong>Solutions :</strong></p>
            <ul className="m-0 pl-5">{double.guidance.map((g) => <li key={g}>{guidanceText[g] ?? g}</li>)}</ul>
          </div>
        )}
      </section>

      <section className="card" aria-label="Jeux testés">
        <h2>Jeux testés</h2>
        <p className="m-0">Aucun jeu n'est encore validé. La matrice (jeu, version, mode, limites) sera remplie après les essais de recette sur trois jeux XInput de typologies différentes. Les jeux protégés par anti-triche peuvent refuser les manettes virtuelles : PadBridge ne cherche pas à contourner ces protections.</p>
      </section>

      <div className="flex gap-2">
        <button className="btn" onClick={() => goto("test")}>Tester ma manette</button>
        <button className="btn" onClick={() => goto("diag")}>Ouvrir les diagnostics</button>
      </div>
    </Page>
  );
}
