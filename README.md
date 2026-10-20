# PadBridge — socle MVP (sans XInput)

Application Windows (Tauri 2 + React/TypeScript + moteur Rust) pour détecter, tester et configurer des manettes DualShock 4 / DualSense. Ce socle couvre les lots 1 et 2 du cahier des charges. Le mode XInput est branché derrière l'interface `VirtualPadBackend` ; tant qu'aucun backend n'est validé (POC B), il reste **indisponible** et l'application fonctionne en mode natif / diagnostic.

Projet indépendant, non affilié à Sony ni à Microsoft. Nom de travail provisoire.

**Portée réelle du mode natif :** Windows réalise l'appairage Bluetooth et expose la manette aux jeux compatibles. PadBridge lit les rapports HID et affiche un aperçu des réglages, mais **ne modifie pas les entrées vues par les jeux en mode natif**. Les transformations de mapping et de zones mortes ne pourront affecter un jeu qu'après intégration et validation d'un backend virtuel adapté. Aucun pilote tiers ni pilote maison n'est installé par cette version.

## Démarrage sur Windows

Prérequis : Node.js 20+, Rust (stable, MSVC), WebView2 (déjà présent sous Windows 11).

```powershell
npm install
npm run tauri icon app-icon.png   # génère src-tauri/icons (obligatoire une seule fois)
npm run tauri dev                 # application complète avec vos manettes
npm run tauri build               # installateur NSIS (installation par utilisateur, sans droits admin)
```

- `npm run dev` seul ouvre l'interface dans un navigateur avec un **moteur simulé** (une DualSense fictive) : pratique pour travailler l'UI.
- Tests du moteur, sans Tauri ni matériel : `cd src-tauri && cargo test --no-default-features` (63 tests).

## Arborescence

```
package.json  vite.config.ts  tsconfig.json  index.html  app-icon.png  .gitignore
src/
  main.tsx  App.tsx  styles.css
  lib/        types.ts  api.ts  mock.ts  i18n.ts  useEngine.ts
  components/ Ui.tsx  Gamepad.tsx  ProfileEditors.tsx
  screens/    Home.tsx  Devices.tsx  TestScreen.tsx  Profiles.tsx
              Compatibility.tsx  Diagnostics.tsx  Settings.tsx
src-tauri/
  Cargo.toml  build.rs  tauri.conf.json  capabilities/default.json
  src/
    main.rs  lib.rs  error.rs  engine.rs
    devices.rs            devices/{model, report_parser, enumeration}.rs
    pipeline.rs           pipeline/{normalize, deadzone, mapping, router}.rs
    virtual_pad.rs        virtual_pad/{backend, null_backend, selected_backend}.rs
    profiles.rs           profiles/{schema, store, migrations}.rs
    diagnostics.rs        diagnostics/{health, double_input, events}.rs
    app.rs                app/{commands, ui_events, lifecycle, permissions}.rs
```

Écarts avec la section 5.5 du cahier des charges : style de modules sans `mod.rs` (`devices.rs` + `devices/`), `app/events.rs` renommé `app/ui_events.rs` (pour ne pas confondre avec `diagnostics/events.rs`), et `error.rs` / `engine.rs` ajoutés à la racine.

## Ce qui est fait (MVP, hors XInput)

| Exigence | État |
|---|---|
| PB-001 diagnostic de démarrage | Fait (Windows, build, Bluetooth, droits, pilote) |
| PB-002/003 installation, réparation, désinstallation | NSIS par utilisateur ; pas de pilote installé par l'app |
| PB-004 niveaux de prise en charge | Fait (écran Compatibilité) |
| PB-005 à PB-010 détection, USB/BT, batterie, perte | Fait, **à valider sur matériel** (POC A) |
| PB-011 à PB-013 tests touches/axes, calibration | Fait, aperçu en direct |
| PB-014/015 mapping simple, profils | Fait (dupliquer, renommer, réinitialiser, corruption → profil sûr) |
| PB-016 mode natif | Fait |
| PB-017/018 XInput + cycle de vie | Logique du moteur faite et testée avec un backend factice ; **aucun backend réel** |
| PB-019/020 double entrée, Steam | Diagnostic et guide ; aucun masquage ni injection |
| PB-022/023 journal, erreurs | Fait (codes PB-Exxx, export expurgé avec consentement) |
| PB-024 accessibilité | Clavier, statuts texte + symbole, thème clair/sombre ; à auditer |
| PB-021 vibration, PB-025 barre système | Non faits (P1) |

## Ce qui reste à faire

1. **POC A (obligatoire avant tout le reste)** : `report_parser.rs` suit les formats HID documentés publiquement, mais n'a pas été testé sur une vraie manette. Vérifiez DS4 et DualSense en USB et Bluetooth. En Bluetooth, la lecture d'un rapport de fonctionnalité au démarrage (`enumeration.rs`) bascule vers le rapport complet ; à confirmer. Les valeurs de batterie sont approximatives.
2. **POC B / ADR** : concevoir et valider un pilote virtuel maison si l'objectif est de maîtriser l'implémentation sans pilote tiers, puis l'intégrer à `selected_backend.rs`. `OutputState::to_xinput()` fournit une représentation des commandes ; cela ne crée pas un périphérique XInput reconnu par Windows. Un périphérique HID virtuel générique n'est pas automatiquement un périphérique Xbox/XInput. La validation en machine de test et la signature du pilote seront indispensables avant distribution.
3. Tests T01 à T24 sur banc réel, trois jeux XInput de référence, revue sécurité et licences.

## Non vérifié ici

Le code Windows (`enumeration.rs`, lecture HID, commandes `cmd`/`powershell`/`tasklist`) n'a pas pu être compilé ni exécuté dans l'environnement de développement (Linux). La couche Tauri compile (`cargo check`) et l'interface a été contrôlée dans un navigateur avec le moteur simulé. Si `cargo build` signale une erreur côté Windows, elle sera dans `devices/enumeration.rs` ou `diagnostics/health.rs`.

## Vibration manuelle DualShock 4 (prototype à valider)

Dans **Test de la manette → Vibrations**, le bouton « Tester la vibration » tente d'envoyer directement un rapport de sortie HID à la DualShock 4, en USB (0x05) ou Bluetooth (0x11 + CRC32), via le **thread de lecture HID déjà ouvert**. Le protocole est implémenté en Rust dans `devices/rumble.rs`, sans pilote supplémentaire ni émulation Xbox. Intensité de l'interface 10 à 60 %, durée 100 à 500 ms, envoi d'un rapport neutre par le thread après l'expiration, à la déconnexion et à l'arrêt de l'application. Cette fonctionnalité n'est **pas encore validée sur une vraie DualShock 4** : une écriture HID acceptée par Windows ne confirme pas une vibration physique. En cas d'erreur, ne tentez pas d'installer un pilote inconnu ; transmettez le code d'erreur affiché.

**Ce n'est pas du retour de force en jeu :** PadBridge n'intercepte pas les effets des jeux et le mode natif reste inchangé. Le support DualSense nécessitera un protocole spécifique distinct.

## Diagnostic de dérive (drift) — sans pilote supplémentaire

Dans **Test de la manette**, cliquez sur **Tester mes joysticks (5 s)**, posez la manette et ne touchez pas aux sticks. PadBridge collecte les axes HID bruts et fait analyser les mesures par le moteur Rust. Le statut peut être normal, dérive possible, mouvement détecté ou incertain. L'outil ne répare pas les joysticks, ne modifie pas les profils et ne constitue pas un diagnostic matériel définitif. Répétez toute mesure suspecte dans des conditions stables.

La batterie est affichée avec le symbole **≈** : la DualShock 4 ne fournit pas une mesure précise à 1 % dans les rapports HID actuellement décodés. Les paliers calculés sont indicatifs.

## Recette sur Windows sans VM

- Les builds Windows et tests Rust sont vérifiés automatiquement par `.github/workflows/windows-ci.yml` sur les pull requests ; les tests physiques restent manuels.
- Le protocole de validation manuelle se trouve dans [`docs/VALIDATION_WINDOWS.md`](docs/VALIDATION_WINDOWS.md).
- Ne jamais installer un pilote noyau expérimental sur le PC principal de développement, surtout quand il ne reste que peu d'espace disque libre.
