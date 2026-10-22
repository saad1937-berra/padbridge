// Test HID manuel DualShock 4, indépendant des vibrations émises par les jeux.
// Durée bornée côté Rust, arrêt explicite en quittant l'écran.
import { useEffect, useRef, useState } from "react";
import { api, toApiError } from "../lib/api";
import type { ControllerDevice, HidOutputInfo } from "../lib/types";

// Gel des impulsions T14 après échec des tests Bluetooth et incident de lecture HID.
// Le diagnostic des capacités reste accessible car il n'écrit rien sur la manette.
const T14_PAUSED = true;

export function RumbleTest({ device, connected }: { device: ControllerDevice; connected: boolean }) {
  const [intensity, setIntensity] = useState(20);
  const [duration, setDuration] = useState(1000);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [inspection, setInspection] = useState<HidOutputInfo | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const activePulse = useRef(false);
  const supported = device.model === "dualshock4" && (device.transport === "bluetooth" || device.transport === "usb");
  const enabled = supported && connected;

  useEffect(() => {
    const id = device.deviceId;
    return () => { if (supported && activePulse.current) void api.stopDs4Rumble(id).catch(() => undefined); };
  }, [device.deviceId, supported]);

  const inspect = async () => {
    setInspecting(true);
    setError(null);
    setInspection(null);
    try {
      setInspection(await api.inspectDs4Hid(device.deviceId));
    } catch (e) {
      const err = toApiError(e);
      setError(`${err.code} : ${err.detail}`);
    } finally {
      setInspecting(false);
    }
  };

  const run = async () => {
    if (T14_PAUSED) return;
    activePulse.current = true;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await api.testDs4Rumble(device.deviceId, intensity, duration);
      setMessage(`Écriture HID acceptée par Windows sur ${(duration / 1000).toLocaleString("fr-FR")} s de test. La vibration physique reste NON CONFIRMÉE : indique si tu l’as réellement ressentie.`);
    } catch (e) {
      const err = toApiError(e);
      setError(`${err.code} : ${err.detail}`);
    } finally {
      activePulse.current = false;
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.stopDs4Rumble(device.deviceId);
      activePulse.current = false;
      setMessage("Commande d'arrêt envoyée.");
    } catch (e) {
      const err = toApiError(e);
      setError(`${err.code} : ${err.detail}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="card flex flex-col gap-3" aria-label="Test de vibration DualShock 4">
      <h2>Vibrations de la manette — test HID</h2>
      <p className="muted m-0">
        Essai volontaire des moteurs de la DualShock 4. Les commandes utilisent le protocole HID
        natif, sans pilote supplémentaire. Aucun effet n'est transmis depuis les jeux.
      </p>
      {T14_PAUSED && (
        <p className="notice notice-warn m-0" role="status">
          T14 — vibrations en pause après échec des essais Bluetooth. Les commandes d'impulsion
          sont désactivées pour préserver la connexion. L'inspection HID en lecture seule reste possible.
        </p>
      )}
      {!supported ? (
        <p className="notice notice-warn m-0">Ce premier prototype ne prend en charge que la DualShock 4 (Bluetooth ou USB).</p>
      ) : (
        <>
          <label className="flex flex-col gap-1">
            <span className="flex justify-between"><span>Intensité du test</span><strong>{intensity} %</strong></span>
            <input type="range" min={10} max={60} step={5} value={intensity}
              onChange={(e) => setIntensity(Number(e.target.value))} disabled={!enabled || busy || T14_PAUSED} />
          </label>
          <label className="flex flex-col gap-1">
            <span className="flex justify-between"><span>Durée demandée</span><strong>{(duration / 1000).toLocaleString("fr-FR")} seconde(s)</strong></span>
            <input type="range" min={500} max={2000} step={250} value={duration}
              onChange={(e) => setDuration(Number(e.target.value))} disabled={!enabled || busy || T14_PAUSED} />
          </label>
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" disabled={!enabled || busy || T14_PAUSED} onClick={() => void run()}>
              {T14_PAUSED ? "Test suspendu (T14)" : busy ? "Impulsion en cours…" : "Tester la vibration"}
            </button>
            <button className="btn" disabled={!enabled || T14_PAUSED} onClick={() => void stop()}>
              Arrêter
            </button>
          </div>
          <div className="flex flex-wrap gap-2">
            <button className="btn" onClick={() => void inspect()} disabled={!enabled || inspecting}>
              {inspecting ? "Inspection en cours…" : "Examiner la sortie HID (sans vibration)"}
            </button>
          </div>
          {inspection && (
            <div className="notice notice-warn" role="status">
              <strong>Capacités HID déclarées par Windows</strong>
              <p className="m-0">Modèle : {inspection.model} · Transport : {inspection.transport}</p>
              <p className="m-0">Rapport testé : {inspection.requestedReportId} · Longueur PadBridge : {inspection.requestedLength} octets</p>
              <p className="m-0">Longueur de sortie HID Windows : {inspection.windowsOutputReportByteLength === null ? "Indisponible" : `${inspection.windowsOutputReportByteLength} octets`}</p>
              {inspection.windowsCapsError && <p className="m-0">Détail inspection : {inspection.windowsCapsError}</p>}
              <p className="muted m-0 text-xs">Windows peut annoncer la taille maximale de la collection, pas seulement celle du rapport 0x11. Cette inspection n'envoie aucune commande motrice.</p>
            </div>
          )}
          {message && <div className="notice notice-warn" role="status">{message}</div>}
          {error && <div className="notice notice-err" role="alert">{error}</div>}
          <p className="muted m-0 text-xs">
            Test limité à 2 secondes par commande, et à 60 % d'intensité dans l'interface.
            Le moteur Rust envoie l'arrêt automatiquement à la fin de la durée choisie, avec une légère marge liée au traitement HID. Clique sur Arrêter en cas de comportement anormal.
            Le succès de l'écriture HID ne garantit pas encore que Windows ou la manette applique la vibration.
          </p>
        </>
      )}
    </section>
  );
}
