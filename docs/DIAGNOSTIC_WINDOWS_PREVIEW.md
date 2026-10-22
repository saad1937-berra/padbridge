# PadBridge — diagnostic NSIS preview (CI interne, sans XInput)

Cette archive est un **build de développement de l'application de diagnostic**, produit par GitHub Actions pour permettre la validation du packaging Windows. **Ce n'est pas la publication de PadBridge au grand public.**

## Ce que cette version fait réellement

- Démarre l'application Windows Tauri avec sa propre identité `app.padbridge.diagnosticpreview`, séparée de la future application publique.
- Affiche les manettes PlayStation vues par Windows, les états des touches/axes, les écrans de profils et diagnostics déjà présents dans PadBridge.
- Peut utiliser la lecture HID existante ; **le mode natif ne modifie pas les commandes reçues par les jeux**.

## Ce qu'elle NE fait PAS

- **Pas de manette Xbox/XInput virtuelle fonctionnelle.** `selected_backend.rs` reste sur `NullBackend`.
- Aucun pilote `.sys`, `.inf` ou `.cat` PadBridge ou tiers n'est inclus par notre processus de packaging ; aucun broker/service n'est installé ou démarré.
- N'installe ni certificat de confiance, ni filtre USB/Bluetooth, ni service de remapping. Le framework Tauri/NSIS peut en revanche utiliser le mécanisme d'installation de WebView2 si celui-ci manque, conformément à sa configuration.
- Ne prouve pas la compatibilité EA FC 26, la vibration, la prise en charge de tous les matériels ni la sécurité d'une version finale.

## Construction

Le workflow `.github/workflows/windows-diagnostic-preview.yml` s'exécute exclusivement comme contrôle de PR, avec permissions GitHub `contents: read` et sans publication de GitHub Release :

```powershell
python scripts/check_public_distribution.py --self-test
python scripts/check_xinput_provider.py --self-test
python scripts/check_diagnostic_preview.py --self-test
npm ci
npm run tauri build -- --ci --bundles nsis --config src-tauri/tauri.diagnostic.conf.json
```

Il produit une archive privée au workflow `PadBridge-Diagnostic-NSIS-Preview-x64` contenant l'installateur **non signé**, un SHA-256 et ce document. L'archive de PR n'est pas un téléchargement grand public et n'a aucune promesse de compatibilité XInput.

## Avant un test manuel futur

- N'installez pas l'archive sur une machine sensible ou sans sauvegarde. Un installateur **non signé** peut déclencher SmartScreen ; **ne désactivez pas SmartScreen ni les protections Windows pour le faire fonctionner**.
- Validez la provenance GitHub Actions et l'empreinte SHA-256, puis faites l'essai sur un environnement Windows approprié, sans driver expérimental.
- Les tests du packaging (réussite du build) ne prouvent pas la qualité de l'installateur en cours d'exécution, sa signature ou sa désinstallation. Ces tests restent distincts et obligatoires.
- Le futur installateur grand public exigera revue du nom/éditeur, licence de PadBridge et dépendances, signature authentifiée, vrais tests matériels et conformité au fichier `policy/public-release.json`.

## Décision de distribution

La gratuité pour tous reste l'objectif. **Aucune release publique n'est autorisée** tant que les quatre bloqueurs de `policy/public-release.json` ne sont pas traités. Un artefact de CI ne doit jamais être annoncé comme un lancement officiel.

**Auteur : Saad Berra** — projet indépendant, non affilié à Sony ni à Microsoft.
