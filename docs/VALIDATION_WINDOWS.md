# PadBridge — validation manuelle Windows

Cette procédure valide le logiciel **sans VM ni installation de pilote expérimental**.

## Préconditions
- Windows 11, PadBridge lancé avec `npm run tauri dev` (pas `npm run dev` seul).
- Manette PlayStation réelle connectée via Bluetooth. Steam Input peut rester désactivé pour le test de diagnostic.
- Aucun pilote virtuel installé par PadBridge : `selected_backend.rs` utilise `NullBackend`.
- Ne pas considérer le fonctionnement d'un jeu sans PadBridge comme la preuve que PadBridge crée une manette virtuelle.

### Diagnostic Windows HID (erreur 87)

L'erreur `PB-E202 : Paramètre incorrect (os error 87)` persiste malgré le tampon de 547 octets déclaré par Windows avec `HidD_SetOutputReport` (méthode de contrôle). Le correctif suivant utilise **`WriteFile` via `HidDevice::write`** (voie de sortie HID), toujours avec le rapport Bluetooth 0x11 de 78 octets, son CRC aux octets 74–77 et le tampon complété à la longueur Windows. Vérifier la CI et effectuer **un seul essai** à 20 % / 1 seconde, relever si Windows renvoie une erreur et si la vibration est physiquement ressentie. Un retour positif de `WriteFile` ne démontre pas que la manette a reçu la commande ; des adaptateurs Bluetooth peuvent accepter silencieusement un rapport sans livraison. Après un retour positif de `WriteFile` sans vibration physique, les flags du rapport ont été alignés sur la séquence rumble-only de SDL : `0x11 0xC4 0x00 0x01 0x00 0x00 <right> <left>`, CRC calculé sur les 74 premiers octets. Refaire au maximum un test bref à faible intensité, puis s'arrêter si l'effet reste absent. Ne pas augmenter l'intensité. Vérifier séparément que les moteurs fonctionnent dans un jeu connu ; cette vérification n'impose aucun logiciel pilote.

## PB-025 — Barre système Windows (validation utilisateur : 20 octobre 2026)
- [x] Démarrer PadBridge : l'icône apparaît dans la zone de notification Windows (parfois sous « Afficher les icônes cachées »).
- [x] Clic droit → « Masquer PadBridge » : fenêtre cachée, processus toujours actif.
- [x] Clic gauche sur l'icône : fenêtre réaffichée et focalisée.
- [x] Clic droit → « Quitter PadBridge » : processus arrêté, lecteur HID arrêté et manette libérée.
- [ ] La croix de la fenêtre **quitte** l'application comme avant : aucun masquage automatique.
- [x] Après fermeture, les boutons de la manette répondent dans `joy.cpl` sans PadBridge ; confirmer avant de relancer un jeu.
- [ ] Aucune nouvelle requête d'autorisation et aucun pilote ajouté.

> Retour utilisateur : essai PB-025 annoncé réussi sur Windows 11. Les cases ci-dessus reflètent la recette demandée (sans télémétrie ni preuve instrumentée). La fermeture par × et l'absence de demandes d'autorisation restent à vérifier séparément.

## T14 — Test de vibration DualShock 4

**SUSPENDU (bouton d'impulsion désactivé dans l'interface)** : la DS4 répond en Bluetooth dans `joy.cpl` et EA FC 26 après redémarrage Windows. Les tests HID `WriteFile` de T14 ne déclenchent pas de vibration physique. Un essai Steam Input a temporairement rendu les commandes inopérantes, rétablies après redémarrage ; la cause précise du blocage n'a pas été établie. **Ne plus lancer de tests de vibration T14 sur le PC principal avant une nouvelle analyse et un banc d'essai approprié.**

- [ ] Depuis la branche de travail, terminer une compilation Windows sans erreur.
- [ ] Connecter la DualShock 4 en Bluetooth, ouvrir **Test de la manette**.
- [ ] Sélectionner intensité **20 %** et durée **1 seconde**, puis cliquer une seule fois sur **Tester la vibration**.
- [ ] Vérifier **physiquement** si la manette vibre et si l'effet cesse rapidement. Si aucune vibration, relever le message exact ; ne pas conclure au succès simplement parce que l'écriture HID a réussi.
- [ ] Essayer **Arrêter** si nécessaire ; aucun effet ne doit persister après une déconnexion ou fermeture.
- [ ] Si la vibration fonctionne : répéter une fois à **30 % / 1,5 seconde** puis vérifier que la lecture des touches continue de fonctionner.
- [ ] Le bouton doit être indisponible pour une DualSense ; aucune installation de pilote n'est autorisée.

## POC A — HID réel
- [ ] L'écran **Appareils** identifie le modèle et le transport Bluetooth.
- [ ] L'écran **Test manette** montre chaque bouton pressé puis relâché.
- [ ] Sticks gauche/droit : axes X/Y atteignent des valeurs proches de -1/+1 et reviennent près de zéro.
- [ ] Gâchettes L2/R2 : variation progressive de 0 à 1.
- [ ] Batteries : valeur plausible ou « Inconnue », jamais une valeur inventée.
- [ ] Éteindre la manette : état déconnecté affiché sans bloquer l'interface.
- [ ] Rallumer et reconnecter : l'application détecte le retour sans redémarrer.
- [ ] Tester USB séparément si un port et un câble de données fiables sont disponibles.
- [ ] Si une valeur est incorrecte, enregistrer modèle, VID/PID, type de transport et code d'erreur, **sans** publier d'identifiant matériel privé.

## Profils et sortie réelle
- [ ] Changer le mapping Croix/Rond : seul l'aperçu calculé doit changer en mode natif.
- [ ] Augmenter une zone morte : l'aperçu se modifie sans modifier la lecture brute.
- [ ] Redémarrer l'application : profil conservé.
- [ ] Bouton XInput désactivé lorsque le backend indique `available: false`.
- [ ] Vérifier que ni l'écran ni le README n'affirment que les commandes calculées sont envoyées au jeu en mode natif.

## Tests automatisés sur Windows
```powershell
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

## Pilote virtuel maison — étape future
Ne pas installer de pilote noyau expérimental sur la machine principale. Concevoir le protocole et un simulateur en espace utilisateur ; vérifier la signature, l'installation, la désinstallation, l'absence de contrôleurs fantômes et la compatibilité réelle avec XInput sur une machine de test dédiée avant toute distribution.
