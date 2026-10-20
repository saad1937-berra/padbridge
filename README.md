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
| PB-011 à PB-013 tests touches/axes, calibration | Fait : aperçu en direct et parcours guidé de 24 commandes HID brutes, **24/24 observées sur DS4 dans la recette utilisateur du 20/10/2026** ; relâchement et précision analogique à vérifier séparément |
| PB-014/015 mapping simple, profils | Fait (dupliquer, renommer, réinitialiser, corruption → profil sûr) |
| PB-016 mode natif | Fait |
| PB-017/018 XInput + cycle de vie | Moteur et simulateur testés ; **POC du pilote maison KMDF/VHF commencé** (`driver/poc-vhf/`), compilation WDK x64 réussie sur GitHub Actions (20/10/2026), pilote non installé ; backend XInput réel toujours absent |
| PB-019/020 double entrée, Steam | Diagnostic et guide ; aucun masquage ni injection |
| PB-022/023 journal, erreurs | Fait (codes PB-Exxx, export expurgé avec consentement) |
| PB-024 accessibilité | Recette ciblée validée sur Windows 11 (20/10/2026) : navigation clavier/focus, annonce à la demande, thèmes, contraste élevé, Narrateur et mise à l'échelle 125 %. Zoom supérieur à 125 %, autres parcours et audit WCAG complet en attente |
| PB-021 vibration Bluetooth (T14) | Suspendu : aucun retour physique malgré WriteFile ; bouton d'impulsion désactivé, inspection HID en lecture seule conservée |
| PB-025 barre système Windows | Implémentée et testée sur Windows 11 par l'utilisateur (20/10/2026) : icône, masquer/afficher/quitter et retour à `joy.cpl` ; points secondaires encore à auditer |

## Ce qui reste à faire

1. **POC A (obligatoire avant tout le reste)** : `report_parser.rs` suit les formats HID documentés publiquement, mais n'a pas été testé sur une vraie manette. Vérifiez DS4 et DualSense en USB et Bluetooth. En Bluetooth, la lecture d'un rapport de fonctionnalité au démarrage (`enumeration.rs`) bascule vers le rapport complet ; à confirmer. Les valeurs de batterie sont approximatives.
2. **POC B / ADR** : concevoir et valider un pilote virtuel maison si l'objectif est de maîtriser l'implémentation sans pilote tiers, puis l'intégrer à `selected_backend.rs`. `OutputState::to_xinput()` fournit une représentation des commandes ; cela ne crée pas un périphérique XInput reconnu par Windows. Un périphérique HID virtuel générique n'est pas automatiquement un périphérique Xbox/XInput. La validation en machine de test et la signature du pilote seront indispensables avant distribution.
3. Tests T01 à T24 sur banc réel, trois jeux XInput de référence, revue sécurité et licences.

## Non vérifié ici

Le code Windows (`enumeration.rs`, lecture HID, commandes `cmd`/`powershell`/`tasklist`) n'a pas pu être compilé ni exécuté dans l'environnement de développement (Linux). La couche Tauri compile (`cargo check`) et l'interface a été contrôlée dans un navigateur avec le moteur simulé. Si `cargo build` signale une erreur côté Windows, elle sera dans `devices/enumeration.rs` ou `diagnostics/health.rs`.

## Vibration manuelle DualShock 4 (prototype à valider)

Dans **Test de la manette → Vibrations**, le bouton « Tester la vibration » tente d'envoyer directement un rapport de sortie HID à la DualShock 4, en USB (0x05) ou Bluetooth (0x11 + CRC32), via le **thread de lecture HID déjà ouvert**. Sous Windows Bluetooth, l'envoi passe par le canal de contrôle HID (`send_output_report`) : un retour positif ne prouve toutefois pas que le moteur physique a réagi. Le protocole est implémenté en Rust dans `devices/rumble.rs`, sans pilote supplémentaire ni émulation Xbox. Intensité de l'interface 10 à 60 %, durée 0,5 à 2 secondes ; l'accusé de fin n'est envoyé à l'interface qu'après la commande neutre de fin de test, à la déconnexion et à l'arrêt de l'application. Cette fonctionnalité n'est **pas encore validée sur une vraie DualShock 4** : une écriture HID acceptée par Windows ne confirme pas une vibration physique. En cas d'erreur, ne tentez pas d'installer un pilote inconnu ; transmettez le code d'erreur affiché.

**Ce n'est pas du retour de force en jeu :** PadBridge n'intercepte pas les effets des jeux et le mode natif reste inchangé. Le support DualSense nécessitera un protocole spécifique distinct.

**Diagnostic PB-E202 / erreur Windows 87 :** le rapport Bluetooth DS4 logique reste de 78 octets (CRC aux octets 74–77) et le tampon Windows utilise la longueur de `HIDP_CAPS.OutputReportByteLength` (547 octets sur la manette testée). Malgré cela, `HidD_SetOutputReport` renvoie l'erreur 87 sur le matériel observé. Le moteur utilise désormais `WriteFile` (`hidapi::HidDevice::write`) avec le rapport Chromium et une durée limitée. Sous certaines combinaisons Bluetooth Windows, `WriteFile` peut accepter un rapport sans que l'appareil le reçoive : seule une vibration physique confirme le succès. **Après un essai sans vibration malgré une écriture acceptée**, nous avons remplacé les indicateurs Bluetooth du rapport `0x11` par le format minimal officiel documenté dans `SDL_hidapi_ps4.c` : octet 1 = `0xC4`, octet 3 = `0x01` (moteurs seuls), moteurs aux octets 6 et 7, CRC aux octets 74–77. Les anciens octets additionnels empruntés à un exemple Chromium sont supprimés. Cette modification est encore **à valider sur matériel**, sans garantie de livraison des paquets Bluetooth. **Aucun pilote tiers n'est nécessaire** pour ce test. En cas d'échec, conserver le code erreur et ne pas répéter les impulsions sans nouveau diagnostic.

## Parcours guidé des entrées HID — POC A

Dans **Test de la manette → Vérification guidée**, démarrer volontairement un parcours de **24 contrôles de base** : 14 boutons, 8 directions des joysticks, 2 gâchettes. Chaque élément n'est marqué « Vérifiée » qu'après observation d'un nouvel échantillon HID brut pendant la session. Le seuil de déclenchement analogique est de 70 % de la course. L'avancement peut être arrêté et remis à zéro ; il n'est pas stocké ni exporté. L'interface exclut les boutons PS, pavé tactile et micro du parcours de base.

**Limites :** les relâchements, l'exactitude des axes, la batterie, l'appairage initial et la reconnexion restent à évaluer séparément. Ce parcours ne modifie aucun profil, ne crée aucune manette virtuelle et ne sollicite pas les moteurs de vibration. Il ne prouve donc pas que les jeux reçoivent un remapping. La procédure de recette est dans `docs/VALIDATION_WINDOWS.md`.

## Pilote virtuel maison — POC B (en développement, pas installable)

Compilation automatisée et contrôles du WDK : [`driver-wdk-ci.yml`](.github/workflows/driver-wdk-ci.yml). [Résultat réussi GitHub Actions](https://github.com/saad1937-berra/padbridge/actions/runs/39510109665). Ce build ne signe, ne charge et n'installe aucun pilote ; l'artefact non signé est conservé seulement 24 heures.

Le nouveau dossier [`driver/poc-vhf/`](driver/poc-vhf/README.md) contient le début d'un **pilote noyau KMDF/VHF en C**, un descripteur de gamepad HID et un IOCTL contrôlé. Le module Rust `virtual_pad/vhf_protocol.rs` construit le rapport binaire de 10 octets (boutons, croix directionnelle, joysticks et gâchettes) avec tests unitaires **sans noyau ni matériel**. Il fournit également `validate_report` et la constante IOCTL. Un **broker Rust préparatoire** a été ajouté dans `vhf_broker.rs` (sessions, neutre a la fermeture, heartbeat, tests simulés) et `vhf_broker_windows.rs` (SetupAPI/DeviceIoControl) : [architecture et précautions](docs/BROKER_POC.md). La partie Windows reste opt-in via `broker-poc`, **sans service installé ni intégration à Tauri**. Le pilote sérialise les envois et tente d'envoyer l'état neutre après 800 ms d'inactivité, à la fermeture du handle et lors de la mise en veille ; ces protections restent **non testées en exécution**.

**Attention :** VHF produit un gamepad HID générique, **pas** une manette Xbox reconnue par XInput. Le prototype C **compile maintenant avec WDK 26100 sur GitHub Actions**, et un binaire `PadBridgeVhf.sys` non signé a été généré (job 39510109665, 20/10/2026). Il dispose désormais d'un **watchdog expérimental à 800 ms** et de callbacks de puissance D0 compilables, mais n'a pas de package INF signé ni de **service broker sécurisé**, n'a pas été chargé en VM et **ne doit pas être installé sur le PC principal**. `selected_backend.rs` reste sur `NullBackend` : l'application et la DualShock 4 continuent de fonctionner comme avant. Les premiers essais du pilote seront réservés à un environnement de test isolé.

## Diagnostic de dérive (drift) — sans pilote supplémentaire

Dans **Test de la manette**, cliquez sur **Tester mes joysticks (5 s)**, posez la manette et ne touchez pas aux sticks. PadBridge collecte les axes HID bruts et fait analyser les mesures par le moteur Rust. Le statut peut être normal, dérive possible, mouvement détecté ou incertain. L'outil ne répare pas les joysticks, ne modifie pas les profils et ne constitue pas un diagnostic matériel définitif. Répétez toute mesure suspecte dans des conditions stables.

La batterie est affichée avec le symbole **≈** : la DualShock 4 ne fournit pas une mesure précise à 1 % dans les rapports HID actuellement décodés. Les paliers calculés sont indicatifs.

## Recette sur Windows sans VM

- Les builds Windows et tests Rust sont vérifiés automatiquement par `.github/workflows/windows-ci.yml` sur les pull requests ; les tests physiques restent manuels.
- Le protocole de validation manuelle se trouve dans [`docs/VALIDATION_WINDOWS.md`](docs/VALIDATION_WINDOWS.md).
- Ne jamais installer un pilote noyau expérimental sur le PC principal de développement, surtout quand il ne reste que peu d'espace disque libre.
