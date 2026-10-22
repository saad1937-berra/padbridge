// src/screens/Home.tsx — accueil : manettes, mode, prérequis, bienvenue.

import { useEffect, useState } from "react";
import { api, toApiError } from "../lib/api";
import { causeText, guidanceText, modeHelp, modelLabel, transportLabel } from "../lib/i18n";
import type {
  ActivateResult, ApiError, AppSettings, ControllerView, DiagnosticsReport, Profile,
} from "../lib/types";
import { ErrorNotice, ModeBadge, Page, StatusBadge, TransportBadge, Yes, batteryText } from "../components/Ui";

export type Screen = "home" | "devices" | "test" | "profiles" | "compat" | "diag" | "settings";

interface Props {
  controllers: ControllerView[];
  loading: boolean;
  refresh: () => Promise<void>;
  profiles: Profile[];
  settings: AppSettings | null;
  updateSettings: (s: AppSettings) => Promise<AppSettings>;
  selectDevice: (id: string) => void;
  goto: (s: Screen) => void;
}

function Prerequisites({ report }: { report: DiagnosticsReport | null }) {
  if (!report) return <p className="muted m-0">Vérification des prérequis…</p>;
  const s = report.system;
  return (
    <table className="data">
      <tbody>
        <tr><th scope="row">Windows</th><td>{s.osVersion ?? "Inconnu"}{s.isWindows11 === false ? " (Windows 10 : à tester)" : ""}</td></tr>
        <tr><th scope="row">Architecture</th><td>{s.arch}</td></tr>
        <tr><th scope="row">Bluetooth</th><td><Yes value={s.bluetoothPresent} /></td></tr>
        <tr><th scope="row">Droits administrateur</th><td>{s.elevated ? "Oui (non nécessaires)" : s.elevated === false ? "Non (suffisant)" : "Inconnu"}</td></tr>
        <tr>
          <th scope="row">Pilote de manette virtuelle</th>
          <td>
            {report.backend.available ? "Disponible" : "Non installé — le mode XInput est désactivé, le mode natif reste disponible."}
          </td>
        </tr>
      </tbody>
    </table>
  );
}

function ControllerCard({
  view, profileName, onChanged, onTest, xinputAvailable,
}: {
  view: ControllerView; profileName: string; onChanged: () => Promise<void>; onTest: () => void;
  xinputAvailable: boolean;
}) {
  const { device, state } = view;
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [result, setResult] = useState<ActivateResult | null>(null);
  const virtualActive = view.virtualSessionId !== null;
  const usable = state.connectionState !== "disconnected";

  const activate = async (mode: "native" | "xinput") => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.activateMode(device.deviceId, mode, null);
      setResult(r);
      if (!r.ok && r.errorCode) setError({ code: r.errorCode, detail: "" });
      setConfirming(false);
      await onChanged();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    if (!view.virtualSessionId) return;
    setBusy(true);
    try {
      await api.stopVirtualSession(view.virtualSessionId);
      setResult(null);
      await onChanged();
    } catch (e) {
      setError(toApiError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <article className="card flex flex-col gap-3" aria-label={modelLabel[device.model]}>
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="m-0 mr-2">{modelLabel[device.model]}</h2>
        <TransportBadge transport={device.transport} />
        <StatusBadge state={state.connectionState} />
        <ModeBadge mode={view.mode} virtualActive={virtualActive} />
      </div>

      <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        <dt className="muted">Batterie</dt><dd className="m-0">{batteryText(state.battery)}</dd>
        <dt className="muted">Profil</dt><dd className="m-0">{profileName}</dd>
        <dt className="muted">Windows voit</dt>
        <dd className="m-0">{modelLabel[device.model]} ({transportLabel[device.transport]})</dd>
        <dt className="muted">Le jeu voit</dt>
        <dd className="m-0">
          {virtualActive ? "Une manette virtuelle liée à cette manette physique (et la manette physique elle-même)" : "La manette physique, telle quelle"}
        </dd>
      </dl>

      {state.connectionState === "unreadable" && (
        <div className="notice notice-err" role="alert">
          Windows voit cette manette mais PadBridge ne peut pas lire ses entrées. Fermez les autres logiciels de manette,
          rebranchez-la ou essayez une manette officielle. Identifiants : {device.vendorId.toString(16).padStart(4, "0")}:
          {device.productId.toString(16).padStart(4, "0")}.
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <button className="btn btn-primary" onClick={onTest} disabled={!usable}>Tester ma manette</button>
        <button className="btn" onClick={() => void activate("native")} disabled={busy || !usable || view.mode === "native"}>
          Mode natif
        </button>
        <button className="btn" onClick={() => setConfirming(true)} disabled={busy || !usable || virtualActive || !xinputAvailable}>
          Compatibilité XInput…
        </button>
        {virtualActive && (
          <button className="btn" onClick={() => void stop()} disabled={busy}>Arrêter la manette virtuelle</button>
        )}
      </div>
      {!xinputAvailable && (
        <p className="muted m-0 text-xs">Mode XInput indisponible : PadBridge ne crée pas encore de manette Xbox virtuelle. Le mode natif fonctionne sans pilote supplémentaire.</p>
      )}

      {confirming && (
        <div className="notice notice-warn" role="group" aria-label="Confirmer le mode XInput">
          <strong>Avant d'activer le mode XInput</strong>
          <p className="m-0 mt-1">{modeHelp.xinput}</p>
          <p className="m-0 mt-1">Vous pourrez revenir au mode natif à tout moment ; la manette virtuelle sera alors retirée.</p>
          <div className="mt-2 flex gap-2">
            <button className="btn btn-primary" onClick={() => void activate("xinput")} disabled={busy}>Activer le mode XInput</button>
            <button className="btn" onClick={() => setConfirming(false)}>Annuler</button>
          </div>
        </div>
      )}

      {error && <ErrorNotice error={error} onClose={() => setError(null)} />}

      {result?.ok && result.doubleInput.risk !== "none" && (
        <div className="notice notice-warn">
          <strong>Double entrée possible</strong>
          <ul className="m-0 mt-1 pl-5">
            {result.doubleInput.causes.map((c) => <li key={c}>{causeText[c] ?? c}</li>)}
          </ul>
          <p className="m-0 mt-2"><strong>Comment résoudre :</strong></p>
          <ul className="m-0 pl-5">
            {result.doubleInput.guidance.map((g) => <li key={g}>{guidanceText[g] ?? g}</li>)}
          </ul>
        </div>
      )}
      {result?.warnings.length ? (
        <div className="notice notice-warn">Le profil appliqué contient des avertissements : consultez l'écran Profils.</div>
      ) : null}
    </article>
  );
}

export function Home({ controllers, loading, refresh, profiles, settings, updateSettings, selectDevice, goto }: Props) {
  const [report, setReport] = useState<DiagnosticsReport | null>(null);

  useEffect(() => {
    let alive = true;
    void api.runDiagnostics().then((r) => alive && setReport(r)).catch(() => undefined);
    return () => { alive = false; };
  }, []);

  const profileName = (id: string) => profiles.find((p) => p.id === id)?.name ?? id;

  return (
    <Page title="Accueil" subtitle="Connectez une manette PS4 ou PS5, vérifiez-la, puis choisissez le mode adapté à votre jeu.">
      {settings && !settings.onboardingDone && (
        <section className="card" aria-label="Bienvenue">
          <h2>Bienvenue dans PadBridge</h2>
          <ol className="m-0 pl-5">
            <li>Reliez la manette en USB (câble de données) ou appairez-la en Bluetooth dans les paramètres Windows.</li>
            <li>Vérifiez qu'elle apparaît ci-dessous, puis cliquez sur « Tester ma manette ».</li>
            <li>Gardez le mode natif si votre jeu reconnaît déjà les manettes PlayStation. Le mode XInput sera disponible après validation de notre pilote virtuel.</li>
          </ol>
          <p className="muted mb-3 mt-3">
            PadBridge ne garantit pas la compatibilité avec tous les jeux (anti-triche, lanceurs…) et n'est affilié ni à Sony ni à Microsoft.
          </p>
          <button className="btn btn-primary" onClick={() => void updateSettings({ ...settings, onboardingDone: true })}>
            J'ai compris
          </button>
        </section>
      )}

      {loading ? (
        <p className="muted">Recherche des manettes…</p>
      ) : controllers.length === 0 ? (
        <section className="card" aria-label="Aucune manette">
          <h2>Aucune manette trouvée</h2>
          <ul className="m-0 pl-5">
            <li><strong>Vérifier le câble :</strong> un câble de charge seule ne transmet pas les données.</li>
            <li><strong>Appairer dans Windows :</strong> Paramètres → Bluetooth et appareils → Ajouter un appareil ; maintenez Share + PS (DualShock 4) ou Create + PS (DualSense) jusqu'au clignotement.</li>
            <li>Pas de Bluetooth sur ce PC ? Utilisez le câble USB.</li>
          </ul>
          <div className="mt-3"><button className="btn" onClick={() => void refresh()}>Réessayer</button></div>
        </section>
      ) : (
        controllers.map((c) => (
          <ControllerCard
            key={c.device.deviceId}
            view={c}
            xinputAvailable={report?.backend.available ?? false}
            profileName={profileName(c.profileId)}
            onChanged={refresh}
            onTest={() => { selectDevice(c.device.deviceId); goto("test"); }}
          />
        ))
      )}

      <section className="card" aria-label="Prérequis">
        <h2>Prérequis système</h2>
        <Prerequisites report={report} />
      </section>
    </Page>
  );
}
