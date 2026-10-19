// src/lib/i18n.ts — chaînes françaises externalisées (NFR-12) et explications d'erreurs (PB-023).

import type {
  ConnectionState, ControllerModel, OutputMode, PsButton, Transport, ValidationIssue, XButton,
} from "./types";

export const modelLabel: Record<ControllerModel, string> = {
  dualshock4: "DualShock 4",
  dualsense: "DualSense",
  dualsenseedge: "DualSense Edge (exploratoire)",
  unknown: "Manette non reconnue",
};

export const transportLabel: Record<Transport, string> = {
  usb: "USB",
  bluetooth: "Bluetooth",
  unknown: "Transport inconnu",
};

export const modeLabel: Record<OutputMode, string> = {
  native: "Natif (HID)",
  xinput: "Compatibilité XInput",
  diagnostic: "Diagnostic uniquement",
};

export const modeHelp: Record<OutputMode, string> = {
  native:
    "Windows voit la manette telle quelle. Aucun pilote supplémentaire. Idéal pour les jeux qui gèrent déjà les manettes PlayStation.",
  xinput:
    "Une manette virtuelle de type Xbox est créée pour les jeux qui n'acceptent que XInput. Nécessite un pilote de manette virtuelle validé. Risque de double entrée si le jeu voit aussi la manette physique.",
  diagnostic:
    "Lecture et tests seulement, sans aucune manette virtuelle.",
};

/** Statuts : texte + symbole, jamais la couleur seule (NFR-11). */
export const stateInfo: Record<ConnectionState, { label: string; icon: string; tone: "ok" | "warn" | "err" | "neutral" }> = {
  connected: { label: "Connectée", icon: "●", tone: "ok" },
  unstable: { label: "Connexion instable", icon: "▲", tone: "warn" },
  unreadable: { label: "Détectée, entrée illisible", icon: "■", tone: "err" },
  disconnected: { label: "Déconnectée", icon: "○", tone: "neutral" },
};

export const psButtonLabel: Record<PsButton, string> = {
  cross: "Croix ✕", circle: "Rond ○", square: "Carré □", triangle: "Triangle △",
  l1: "L1", r1: "R1", l3: "L3 (clic stick gauche)", r3: "R3 (clic stick droit)",
  share: "Share / Create", options: "Options", ps: "Bouton PS", touchpad: "Pavé tactile", mute: "Micro (Mute)",
  dpad_up: "Croix directionnelle ↑", dpad_down: "Croix directionnelle ↓",
  dpad_left: "Croix directionnelle ←", dpad_right: "Croix directionnelle →",
};

export const xButtonLabel: Record<XButton, string> = {
  A: "A", B: "B", X: "X", Y: "Y", LB: "LB", RB: "RB", Back: "Back", Start: "Start", Guide: "Guide",
  LS: "LS (clic stick gauche)", RS: "RS (clic stick droit)",
  DpadUp: "Croix ↑", DpadDown: "Croix ↓", DpadLeft: "Croix ←", DpadRight: "Croix →",
};

export interface ErrorText { title: string; explanation: string; action: string }

const errors: Record<string, ErrorText> = {
  "PB-E001": {
    title: "Demande invalide",
    explanation: "L'interface a envoyé une valeur que le moteur refuse.",
    action: "Réessayez ; si cela se reproduit, exportez un diagnostic.",
  },
  "PB-E002": {
    title: "Erreur interne",
    explanation: "Le moteur a rencontré un problème inattendu.",
    action: "Réessayez, puis exportez un diagnostic depuis l'écran Diagnostics.",
  },
  "PB-E101": {
    title: "Pilote de manette virtuelle indisponible",
    explanation:
      "Le mode XInput a besoin d'un pilote de manette virtuelle validé, et aucun n'est installé.",
    action:
      "Restez en mode natif (toujours disponible) ou consultez l'écran Compatibilité pour installer un pilote volontairement.",
  },
  "PB-E102": {
    title: "La manette virtuelle a cessé de répondre",
    explanation: "Le pilote a refusé une mise à jour ; PadBridge est revenu au mode natif par sécurité.",
    action: "Réactivez le mode XInput. Si le problème persiste, exportez un diagnostic.",
  },
  "PB-E201": {
    title: "Manette introuvable",
    explanation: "La manette a été débranchée ou n'est plus visible par Windows.",
    action: "Vérifiez le câble de données ou l'appairage Bluetooth, puis réessayez.",
  },
  "PB-E202": {
    title: "Manette détectée mais illisible",
    explanation:
      "Windows voit la manette, mais PadBridge n'a pas pu lire ses entrées (accès refusé, autre logiciel, clone non compatible).",
    action: "Fermez les autres logiciels de manette, rebranchez-la, ou testez avec une manette officielle.",
  },
  "PB-E203": {
    title: "Rapport de manette non compris",
    explanation: "La manette a envoyé un format de données que PadBridge ne connaît pas.",
    action: "Exportez un diagnostic en indiquant le modèle exact et le mode de connexion.",
  },
  "PB-E301": {
    title: "Profil invalide",
    explanation: "Une valeur du profil est hors limites ou incorrecte.",
    action: "Corrigez le champ signalé puis enregistrez de nouveau.",
  },
  "PB-E302": {
    title: "Profil introuvable",
    explanation: "Le profil demandé n'existe plus.",
    action: "Choisissez un autre profil dans la liste.",
  },
  "PB-E303": {
    title: "Profil corrompu",
    explanation: "Le fichier du profil était illisible. Une copie a été conservée pour le support.",
    action: "Utilisez le profil par défaut ou recréez le profil.",
  },
  "PB-E304": {
    title: "Erreur de stockage",
    explanation: "PadBridge n'a pas pu lire ou écrire ses fichiers locaux.",
    action: "Vérifiez l'espace disque et les droits du dossier de données.",
  },
  "PB-E401": {
    title: "Export impossible",
    explanation: "Le fichier de diagnostic n'a pas pu être écrit à cet emplacement.",
    action: "Choisissez un autre dossier et un nom de fichier se terminant par .txt.",
  },
};

export function errorText(code: string): ErrorText {
  return (
    errors[code] ?? {
      title: "Erreur",
      explanation: "Une erreur est survenue.",
      action: "Réessayez, puis exportez un diagnostic si besoin.",
    }
  );
}

export function issueText(issue: ValidationIssue): string {
  switch (issue.code) {
    case "NAME_EMPTY": return "Le nom du profil ne peut pas être vide.";
    case "NAME_TOO_LONG": return "Le nom du profil est trop long (64 caractères maximum).";
    case "ID_INVALID": return "Identifiant de profil invalide.";
    case "UNSUPPORTED_SCHEMA": return "Version de profil non prise en charge.";
    case "DEADZONE_OUT_OF_RANGE": return `Valeur hors limites (${issue.detail ?? "0 – 0,5"}).`;
    case "ACTION_UNREACHABLE":
      return `L'action « ${issue.detail ?? "?"} » n'est plus atteignable avec ce mapping : aucune touche ne l'envoie.`;
    case "LANGUAGE_UNSUPPORTED": return "Langue non disponible pour l'instant.";
    case "THEME_INVALID": return "Thème invalide.";
    case "RETENTION_OUT_OF_RANGE": return "La durée de conservation doit être comprise entre 1 et 90 jours.";
    default: return `${issue.field} : ${issue.code}`;
  }
}

export const causeText: Record<string, string> = {
  PHYSICAL_AND_VIRTUAL:
    "La manette physique et la manette virtuelle sont toutes deux visibles : certains jeux peuvent recevoir chaque commande deux fois.",
  STEAM_RUNNING:
    "Steam est ouvert : si Steam Input est actif pour cette manette, il peut ajouter une troisième source de commandes.",
};

export const guidanceText: Record<string, string> = {
  RETURN_TO_NATIVE: "Revenir au mode natif (bouton ci-dessus) : c'est la solution la plus sûre.",
  DISABLE_STEAM_INPUT_FOR_CONTROLLER:
    "Dans Steam : Paramètres → Manette → désactivez la prise en charge de votre manette, ou désactivez Steam Input pour le jeu concerné.",
  CHECK_GAME_INPUT_SETTINGS:
    "Dans les options du jeu, choisissez une seule manette ou une seule source d'entrée.",
  OPTIONAL_HID_FILTER:
    "Avancé : un filtre HID validé pourrait masquer la manette physique. PadBridge ne l'active jamais automatiquement.",
};

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleTimeString("fr-FR");
}
