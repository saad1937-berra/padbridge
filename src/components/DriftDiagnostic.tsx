// Diagnostic volontaire de centrage des sticks : l'utilisateur ne doit pas les toucher.
// La collecte s'effectue sur les axes BRUTS, indépendamment des profils/zones mortes.
import { useEffect, useRef, useState } from "react";
import { api, toApiError } from "../lib/api";
import type { DriftReport, DriftSample, DriftStatus, InputSample } from "../lib/types";

type Phase = "idle" | "recording" | "analyzing";
const DURATION_MS = 5000;

const verdict: Record<DriftStatus, string> = {
  normal: "Aucune dérive notable observée",
  possible_drift: "Décalage persistant : dérive possible",
  movement_detected: "Mouvement important détecté : recommencer sans toucher le stick",
  inconclusive: "Résultat incertain : refaire le test",
};

function Result({ name, status, meanMagnitude, peakMagnitude }: {
  name: string;
  status: DriftStatus;
  meanMagnitude: number;
  peakMagnitude: number;
}) {
  return (
    <div className="flex flex-col gap-1">
      <strong>{name} — {verdict[status]}</strong>
      <p className="muted m-0 text-sm">
        Écart moyen : {Math.round(meanMagnitude * 100)} % · écart maximal : {Math.round(peakMagnitude * 100)} %
      </p>
    </div>
  );
}

export function DriftDiagnostic({ deviceId, sample }: {
  deviceId: string;
  sample: InputSample | null;
}) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [remaining, setRemaining] = useState(5);
  const [count, setCount] = useState(0);
  const [report, setReport] = useState<DriftReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const samples = useRef<DriftSample[]>([]);
  const lastSeq = useRef<number | null>(null);
  const startedAt = useRef(0);
  const generation = useRef(0);

  // Changement de manette : abandonner immédiatement le diagnostic en cours.
  useEffect(() => {
    generation.current += 1;
    samples.current = [];
    lastSeq.current = null;
    setPhase("idle");
    setReport(null);
    setError(null);
  }, [deviceId]);

  useEffect(() => {
    if (phase !== "recording" || !sample || sample.deviceId !== deviceId) return;
    if (sample.seq === lastSeq.current || sample.timestampUs === 0) return;
    lastSeq.current = sample.seq;
    if (samples.current.length >= 240) return;
    samples.current.push({
      left: { ...sample.leftRaw },
      right: { ...sample.rightRaw },
    });
    setCount(samples.current.length);
  }, [sample, phase, deviceId]);

  useEffect(() => {
    if (phase !== "recording") return;
    const token = generation.current;
    const interval = window.setInterval(() => {
      const left = Math.max(0, DURATION_MS - (Date.now() - startedAt.current));
      setRemaining(Math.ceil(left / 1000));
      if (left > 0) return;

      window.clearInterval(interval);
      setPhase("analyzing");
      const recorded = [...samples.current];
      if (recorded.length < 30) {
        setError("Trop peu de données HID reçues. Vérifie la connexion Bluetooth, puis recommence.");
        setPhase("idle");
        return;
      }
      void api.analyzeStickDrift(recorded).then((r) => {
        if (generation.current === token) setReport(r);
      }).catch((e) => {
        if (generation.current === token) setError(toApiError(e).detail || "Analyse impossible.");
      }).finally(() => {
        if (generation.current === token) setPhase("idle");
      });
    }, 100);
    return () => window.clearInterval(interval);
  }, [phase]);

  const start = () => {
    generation.current += 1;
    samples.current = [];
    lastSeq.current = sample?.seq ?? null;
    startedAt.current = Date.now();
    setCount(0);
    setRemaining(5);
    setReport(null);
    setError(null);
    setPhase("recording");
  };

  const cancel = () => {
    generation.current += 1;
    setPhase("idle");
    setError(null);
  };

  return (
    <section className="card flex flex-col gap-3" aria-label="Diagnostic de dérive des joysticks">
      <h2>Diagnostic de dérive (drift)</h2>
      <p className="muted m-0">
        Pose la manette à plat et <strong>ne touche à aucun joystick</strong> pendant les 5 secondes de mesure.
        PadBridge analyse les positions brutes, sans appliquer les zones mortes.
      </p>
      <div className="flex flex-wrap items-center gap-3">
        {phase === "idle" ? (
          <button className="btn btn-primary" onClick={start} disabled={!sample}>Tester mes joysticks (5 s)</button>
        ) : (
          <button className="btn" onClick={cancel}>Annuler</button>
        )}
        {phase === "recording" && (
          <>
            <span>Mesure en cours… {remaining} s · {count} échantillons</span>
            <span className="sr-only" role="status">Mesure en cours pour cinq secondes. Ne touchez pas aux joysticks.</span>
          </>
        )}
        {phase === "analyzing" && <span role="status">Analyse Rust en cours…</span>}
      </div>
      {error && <div className="notice notice-warn" role="alert">{error}</div>}
      {report && (
        <div className="notice notice-ok flex flex-col gap-2" role="status">
          <strong>Analyse de {report.sampleCount} échantillons</strong>
          <Result name="Stick gauche" {...report.left} />
          <Result name="Stick droit" {...report.right} />
        </div>
      )}
      <p className="muted m-0 text-xs">
        Test indicatif : un mouvement volontaire, des parasites ou une déviation intermittente peuvent fausser le résultat.
        Un résultat suspect doit être confirmé avec plusieurs mesures. Aucun paramètre n'est changé automatiquement.
      </p>
    </section>
  );
}
