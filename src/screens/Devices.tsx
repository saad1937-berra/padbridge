// src/screens/Devices.tsx — liste des appareils et détails techniques.

import { useState } from "react";
import { modelLabel, formatDate } from "../lib/i18n";
import type { ControllerView } from "../lib/types";
import { Page, StatusBadge, TransportBadge, batteryText } from "../components/Ui";

const hex = (n: number) => n.toString(16).padStart(4, "0");

export function Devices({
  controllers, refresh, selectedId, selectDevice,
}: {
  controllers: ControllerView[];
  refresh: () => Promise<void>;
  selectedId: string | null;
  selectDevice: (id: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const selected = controllers.find((c) => c.device.deviceId === selectedId) ?? controllers[0] ?? null;

  const reload = async () => {
    setBusy(true);
    try { await refresh(); } finally { setBusy(false); }
  };

  return (
    <Page title="Appareils" subtitle="Manettes vues par PadBridge, avec leur transport, leur état et leur batterie.">
      <div><button className="btn" onClick={() => void reload()} disabled={busy}>Rafraîchir</button></div>

      {controllers.length === 0 ? (
        <p className="muted">Aucun appareil. Branchez une manette ou appairez-la dans Windows.</p>
      ) : (
        <div className="card p-0">
          <table className="data">
            <thead>
              <tr><th>Modèle</th><th>Transport</th><th>État</th><th>Batterie</th><th></th></tr>
            </thead>
            <tbody>
              {controllers.map((c) => (
                <tr key={c.device.deviceId} aria-selected={selected?.device.deviceId === c.device.deviceId}>
                  <td>{modelLabel[c.device.model]}</td>
                  <td><TransportBadge transport={c.device.transport} /></td>
                  <td><StatusBadge state={c.state.connectionState} /></td>
                  <td>{batteryText(c.state.battery)}</td>
                  <td>
                    <button className="btn" onClick={() => selectDevice(c.device.deviceId)}>Détails</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {selected && (
        <section className="card" aria-label="Détails de l'appareil">
          <h2>Détails — {modelLabel[selected.device.model]}</h2>
          <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
            <dt className="muted">Identifiants matériels (VID:PID)</dt>
            <dd className="m-0">{hex(selected.device.vendorId)}:{hex(selected.device.productId)}</dd>
            <dt className="muted">Identifiant local</dt>
            <dd className="m-0">{selected.device.deviceId} <span className="muted">(pseudonyme, pas un numéro de série)</span></dd>
            <dt className="muted">Dernière détection</dt>
            <dd className="m-0">{formatDate(selected.device.lastSeenMs)}</dd>
            <dt className="muted">Capacités du modèle</dt>
            <dd className="m-0">
              {[
                selected.device.capabilityFlags.touchpad && "pavé tactile",
                selected.device.capabilityFlags.gyroscope && "gyroscope",
                selected.device.capabilityFlags.muteButton && "bouton micro",
                selected.device.capabilityFlags.adaptiveTriggers && "gâchettes adaptatives",
                selected.device.capabilityFlags.lightbar && "barre lumineuse",
              ].filter(Boolean).join(", ") || "aucune connue"}
            </dd>
          </dl>
          <p className="muted mb-0 mt-3 text-xs">
            Ces capacités sont celles du matériel. PadBridge v1.0 ne les expose pas toutes : voir l'écran Compatibilité.
          </p>
        </section>
      )}
    </Page>
  );
}
