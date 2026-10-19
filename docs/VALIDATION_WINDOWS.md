# PadBridge — validation manuelle Windows

Cette procédure valide le logiciel **sans VM ni installation de pilote expérimental**.

## Préconditions
- Windows 11, PadBridge lancé avec `npm run tauri dev` (pas `npm run dev` seul).
- Manette PlayStation réelle connectée via Bluetooth. Steam Input peut rester désactivé pour le test de diagnostic.
- Aucun pilote virtuel installé par PadBridge : `selected_backend.rs` utilise `NullBackend`.
- Ne pas considérer le fonctionnement d'un jeu sans PadBridge comme la preuve que PadBridge crée une manette virtuelle.

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
