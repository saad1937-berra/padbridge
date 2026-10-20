// Test HID manuel DualShock 4, indépendant des vibrations émises par les jeux.
// Durée bornée côté Rust, arrêt explicite en quittant l'écran.
import { useEffect, useState } from "react";
import { api, toApiError } from "../lib/api";
import type { ControllerDevice } from "../lib/types";

export function RumbleTest({ device, connected }: { device: ControllerDevice; connected: boolean }) {
  const [intensity, setIntensity] = useState(20);
  const [duration, setDuration] = useState(200);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const supported = device.model === "dualshock4" && (device.transport === "bluetooth" || device.transport === "usb");
  const enabled = supported && connected;

  useEffect(() => {
    const id = device.deviceId;
    return () => { if (supported) void api.stopDs4Rumble(id).catch(() => undefined); };
  }, [device.deviceId, supported]);

  const run = async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await api.testDs4Rumble(device.deviceId, intensity, duration);
      setMessage("Commande HID écrite. La manette devrait vibrer brièvement : confirme-le physiquement.");
    } catch (e) {
      const err = toApiError(e);
      setError(`${err.code} : ${err.detail}`);
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.stopDs4Rumble(device.deviceId);
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
      {!supported ? (
        <p className="notice notice-warn m-0">Ce premier prototype ne prend en charge que la DualShock 4 (Bluetooth ou USB).</p>
      ) : (
        <>
          <label className="flex flex-col gap-1">
            <span className="flex justify-between"><span>Intensité du test</span><strong>{intensity} %</strong></span>
            <input type="range" min={10} max={60} step={5} value={intensity}
              onChange={(e) => setIntensity(Number(e.target.value))} disabled={!enabled || busy} />
          </label>
          <label className="flex flex-col gap-1">
            <span className="flex justify-between"><span>Durée de l'impulsion</span><strong>{duration} ms</strong></span>
            <input type="range" min={100} max={500} step={50} value={duration}
              onChange={(e) => setDuration(Number(e.target.value))} disabled={!enabled || busy} />
          </label>
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" disabled={!enabled || busy} onClick={() => void run()}>
              Tester la vibration
            </button>
            <button className="btn" disabled={!enabled || busy} onClick={() => void stop()}>
              Arrêter
            </button>
          </div>
          {message && <div className="notice notice-ok" role="status">{message}</div>}
          {error && <div className="notice notice-err" role="alert">{error}</div>}
          <p className="muted m-0 text-xs">
            Test limité à 500 ms par commande, et à 60 % d'intensité dans l'interface.
            Un arrêt automatique est envoyé depuis Rust. Arrête le test en cas de comportement anormal.
            Le succès de l'écriture HID ne garantit pas encore que Windows ou la manette applique la vibration.
          </p>
        </>
      )}
    </section>
  );
}
