// src/components/Gamepad.tsx — schéma générique d'une manette (formes neutres, sans logo).
// Une pièce active est remplie ET plus épaisse : l'état ne repose pas sur la couleur seule.

import { useState } from "react";
import type { InputSample, PsButton } from "../lib/types";
import { psButtonLabel } from "../lib/i18n";

const on = (s: InputSample | null, b: PsButton) => !!s?.pressed.includes(b);

export function Gamepad({ sample }: { sample: InputSample | null }) {
  const [announcement, setAnnouncement] = useState("");
  const lx = sample?.leftRaw.x ?? 0;
  const ly = sample?.leftRaw.y ?? 0;
  const rx = sample?.rightRaw.x ?? 0;
  const ry = sample?.rightRaw.y ?? 0;
  const pressed = sample?.pressed ?? [];

  const Face = ({ cx, cy, b, glyph }: { cx: number; cy: number; b: PsButton; glyph: string }) => (
    <g>
      <circle className="pad-part" data-on={on(sample, b)} cx={cx} cy={cy} r={17} />
      <text x={cx} y={cy + 6} textAnchor="middle" fontSize="18" fill="currentColor" aria-hidden="true">
        {glyph}
      </text>
    </g>
  );

  const Pill = ({ x, y, w, b, label }: { x: number; y: number; w: number; b: PsButton; label: string }) => (
    <g>
      <rect className="pad-part" data-on={on(sample, b)} x={x} y={y} width={w} height={20} rx={10} />
      <text x={x + w / 2} y={y + 14} textAnchor="middle" fontSize="12" fill="currentColor" aria-hidden="true">
        {label}
      </text>
    </g>
  );

  const Dpad = ({ x, y, b, label }: { x: number; y: number; b: PsButton; label: string }) => (
    <g>
      <rect className="pad-part" data-on={on(sample, b)} x={x} y={y} width={24} height={24} rx={4} />
      <text x={x + 12} y={y + 17} textAnchor="middle" fontSize="14" fill="currentColor" aria-hidden="true">
        {label}
      </text>
    </g>
  );

  return (
    <div>
      <svg viewBox="0 0 520 300" role="img" aria-label="Schéma de la manette" style={{ width: "100%", maxWidth: 560, color: "var(--text)" }}>
        <path
          d="M110 80 C150 60 370 60 410 80 C470 100 500 200 470 250 C450 285 410 275 385 240 L345 200 L175 200 L135 240 C110 275 70 285 50 250 C20 200 50 100 110 80 Z"
          fill="var(--surface)"
          stroke="var(--border)"
          strokeWidth={3}
        />
        {/* gâchettes analogiques */}
        <rect x={95} y={20} width={90} height={14} rx={4} fill="var(--surface-2)" stroke="var(--muted)" />
        <rect x={95} y={20} width={90 * (sample?.l2Raw ?? 0)} height={14} rx={4} fill="var(--accent)" />
        <text x={140} y={14} textAnchor="middle" fontSize="11" fill="currentColor">L2</text>
        <rect x={335} y={20} width={90} height={14} rx={4} fill="var(--surface-2)" stroke="var(--muted)" />
        <rect x={335} y={20} width={90 * (sample?.r2Raw ?? 0)} height={14} rx={4} fill="var(--accent)" />
        <text x={380} y={14} textAnchor="middle" fontSize="11" fill="currentColor">R2</text>

        <Pill x={95} y={42} w={90} b="l1" label="L1" />
        <Pill x={335} y={42} w={90} b="r1" label="R1" />

        <Dpad x={98} y={98} b="dpad_up" label="↑" />
        <Dpad x={98} y={146} b="dpad_down" label="↓" />
        <Dpad x={74} y={122} b="dpad_left" label="←" />
        <Dpad x={122} y={122} b="dpad_right" label="→" />

        <Face cx={420} cy={96} b="triangle" glyph="△" />
        <Face cx={420} cy={148} b="cross" glyph="✕" />
        <Face cx={394} cy={122} b="square" glyph="□" />
        <Face cx={446} cy={122} b="circle" glyph="○" />

        <Pill x={190} y={92} w={50} b="share" label="Share" />
        <Pill x={280} y={92} w={50} b="options" label="Opt." />
        <rect className="pad-part" data-on={on(sample, "touchpad")} x={215} y={120} width={90} height={34} rx={8} />
        <text x={260} y={141} textAnchor="middle" fontSize="11" fill="currentColor">Pavé</text>
        <circle className="pad-part" data-on={on(sample, "ps")} cx={260} cy={176} r={11} />
        <text x={260} y={180} textAnchor="middle" fontSize="10" fill="currentColor">PS</text>

        {/* sticks : cercle fixe + point mobile ; L3/R3 = clic */}
        <circle className="pad-part" data-on={on(sample, "l3")} cx={185} cy={210} r={36} />
        <circle cx={185 + lx * 24} cy={210 - ly * 24} r={11} fill="var(--text)" />
        <circle className="pad-part" data-on={on(sample, "r3")} cx={335} cy={210} r={36} />
        <circle cx={335 + rx * 24} cy={210 - ry * 24} r={11} fill="var(--text)" />
      </svg>
      <p className="m-0 mt-2">
        <strong>Boutons actifs :</strong>{" "}
        {pressed.length ? pressed.map((b) => psButtonLabel[b]).join(", ") : <span className="muted">aucun</span>}
      </p>
      <button className="btn mt-2" type="button" onClick={() => setAnnouncement(pressed.length ? `Boutons actifs : ${pressed.map((b) => psButtonLabel[b]).join(", ")}` : "Aucun bouton actif")}>Annoncer les boutons actifs</button>
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">{announcement}</span>
    </div>
  );
}
