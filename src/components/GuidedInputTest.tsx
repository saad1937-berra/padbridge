// POC A : recette interactive des commandes HID en lecture seule.
// N'envoie aucune donnée à la manette et ne modifie aucun profil.
import { useEffect, useRef, useState } from "react";
import { psButtonLabel } from "../lib/i18n";
import type { InputSample, PsButton } from "../lib/types";

type Group = "Touches" | "Joysticks" | "Gâchettes";
interface Check {
  id: string;
  label: string;
  group: Group;
}

const BUTTONS: PsButton[] = [
  "cross", "circle", "square", "triangle",
  "dpad_up", "dpad_down", "dpad_left", "dpad_right",
  "l1", "r1", "l3", "r3", "share", "options",
];

const CHECKS: Check[] = [
  ...BUTTONS.map((button): Check => ({
    id: `button:${button}`,
    label: psButtonLabel[button],
    group: "Touches",
  })),
  { id: "left_x_neg", label: "Stick gauche vers la gauche", group: "Joysticks" },
  { id: "left_x_pos", label: "Stick gauche vers la droite", group: "Joysticks" },
  { id: "left_y_pos", label: "Stick gauche vers le haut", group: "Joysticks" },
  { id: "left_y_neg", label: "Stick gauche vers le bas", group: "Joysticks" },
  { id: "right_x_neg", label: "Stick droit vers la gauche", group: "Joysticks" },
  { id: "right_x_pos", label: "Stick droit vers la droite", group: "Joysticks" },
  { id: "right_y_pos", label: "Stick droit vers le haut", group: "Joysticks" },
  { id: "right_y_neg", label: "Stick droit vers le bas", group: "Joysticks" },
  { id: "trigger_l2", label: "Appuyer progressivement sur L2", group: "Gâchettes" },
  { id: "trigger_r2", label: "Appuyer progressivement sur R2", group: "Gâchettes" },
];

const AXIS_THRESHOLD = 0.7;

/** Retourne uniquement les commandes observées sur cette trame HID brute. */
export function checksFromSample(sample: InputSample): string[] {
  const observed = sample.pressed
    .filter((button) => BUTTONS.includes(button))
    .map((button) => `button:${button}`);

  if (sample.leftRaw.x <= -AXIS_THRESHOLD) observed.push("left_x_neg");
  if (sample.leftRaw.x >= AXIS_THRESHOLD) observed.push("left_x_pos");
  if (sample.leftRaw.y >= AXIS_THRESHOLD) observed.push("left_y_pos");
  if (sample.leftRaw.y <= -AXIS_THRESHOLD) observed.push("left_y_neg");
  if (sample.rightRaw.x <= -AXIS_THRESHOLD) observed.push("right_x_neg");
  if (sample.rightRaw.x >= AXIS_THRESHOLD) observed.push("right_x_pos");
  if (sample.rightRaw.y >= AXIS_THRESHOLD) observed.push("right_y_pos");
  if (sample.rightRaw.y <= -AXIS_THRESHOLD) observed.push("right_y_neg");
  if (sample.l2Raw >= AXIS_THRESHOLD) observed.push("trigger_l2");
  if (sample.r2Raw >= AXIS_THRESHOLD) observed.push("trigger_r2");

  return observed;
}

export function GuidedInputTest({ deviceId, sample }: {
  deviceId: string;
  sample: InputSample | null;
}) {
  const [running, setRunning] = useState(false);
  const [seen, setSeen] = useState<string[]>([]);
  const lastSeq = useRef<number | null>(null);
  const usable = sample?.deviceId === deviceId && sample.timestampUs > 0;
  const complete = seen.length === CHECKS.length;

  useEffect(() => {
    if (!running || !sample || sample.deviceId !== deviceId || sample.timestampUs === 0) return;
    if (sample.seq === lastSeq.current) return;
    lastSeq.current = sample.seq;
    const observed = checksFromSample(sample);
    if (!observed.length) return;
    setSeen((previous) => {
      const next = new Set(previous);
      for (const id of observed) next.add(id);
      return next.size === previous.length ? previous : [...next];
    });
  }, [deviceId, running, sample]);

  const start = () => {
    lastSeq.current = sample?.seq ?? null; // Ne pas compter l'échantillon précédent le clic.
    setSeen([]);
    setRunning(true);
  };

  return (
    <section className="card flex flex-col gap-3" aria-label="Vérification guidée des commandes">
      <h2>Vérification guidée — boutons, sticks et gâchettes</h2>
      <p className="muted m-0">
        Démarrez le parcours, puis appuyez sur les touches et déplacez chaque stick dans les quatre
        directions. Les commandes détectées passent à « Vérifiée ». La mesure utilise uniquement
        les données HID brutes : aucun pilote, vibration ni modification du profil.
      </p>
      <p className="muted m-0 text-xs">
        Pour les directions et gâchettes, le seuil de détection est de 70 % de la course.
        Ce parcours constate les mouvements reçus ; il ne prouve pas la précision du calibrage,
        le relâchement des touches ni la compatibilité avec un jeu. Les boutons PS, pavé tactile
        et micro ne sont pas requis dans ce parcours de base.
      </p>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn btn-primary" disabled={!usable} onClick={start}>
          {seen.length ? "Recommencer le parcours" : "Démarrer le parcours"}
        </button>
        <button type="button" className="btn" disabled={!running} onClick={() => setRunning(false)}>
          Terminer le parcours
        </button>
      </div>
      <div role="status" aria-live="polite" aria-atomic="true">
        <strong>{seen.length} / {CHECKS.length} commandes vérifiées</strong>
        <span className="muted"> — {complete ? "Toutes les commandes de base ont été observées" : running ? "Test en cours" : seen.length ? "Parcours arrêté (résultat partiel)" : "Prêt à démarrer"}</span>
      </div>
      <progress className="w-full" value={seen.length} max={CHECKS.length} aria-label="Progression du test guidé" />
      <div className="grid gap-4 lg:grid-cols-3">
        {(["Touches", "Joysticks", "Gâchettes"] as const).map((group) => (
          <div key={group}>
            <h3>{group}</h3>
            <ul className="m-0 list-none space-y-1 p-0">
              {CHECKS.filter((check) => check.group === group).map((check) => {
                const ok = seen.includes(check.id);
                return (
                  <li key={check.id} className="flex gap-2">
                    <span aria-hidden="true">{ok ? "✓" : "○"}</span>
                    <span>{check.label} — {ok ? "Vérifiée" : "À vérifier"}</span>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
      {complete && <p className="notice notice-ok m-0" role="status">Parcours terminé : toutes les commandes de base ont été observées pendant cette session. Résultat non enregistré.</p>}
    </section>
  );
}
