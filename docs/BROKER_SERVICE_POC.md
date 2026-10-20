# Service broker PadBridge — prototype IPC local V1, inactif par defaut

## Situation

Cette etape ajoute le **protocole IPC** et une **fondation d'ecoute Windows**
sur pipe nomme, pas encore un Windows Service enregistre dans le SCM.

- `src-tauri/src/virtual_pad/broker_ipc.rs` : trame fixe de **18 octets**,
  version 1, commandes PING/NEUTRAL/SUBMIT/CLOSE, reponse fixe de 8 octets.
  Toute trame malformee, mauvaise version, octets reserves ou rapport invalide
  est rejetee. Aucun mot de passe ou identite fourni par le client ne fait foi.
- `broker_pipe_windows.rs` : nom fixe
  `\\\\.\\pipe\\PadBridge.Broker.POC.v1`, une instance exclusive,
  acces uniquement local (`PIPE_REJECT_REMOTE_CLIENTS`), ACL protegee
  Windows (SYSTEM, Administrateurs et **SID numerique explicitement autorise**).
  Le droit du SID utilisateur est `0x00120083` : lire/ecrire/synchroniser,
  **sans `FILE_CREATE_PIPE_INSTANCE`**. Pas de droit Everyone.
- `src-tauri/src/bin/padbridge-broker-preview.rs` : programme de
  demonstration *one-shot* derriere l'option de compilation
  `broker-service-poc`. Refuse tout demarrage sans parametre explicite.
  Il repond a **un seul** message et se termine sans toucher au pilote.

## Limites volontaires : aucun controle virtuel actif

Le gestionnaire IPC appelle **uniquement** `handle_dry_run` :

| Requete | Reponse |
| --- | --- |
| PING | OK |
| CLOSE | OK |
| NEUTRAL | Disabled |
| SUBMIT, meme valide | Disabled |
| Invalide / longueur incorrecte | Malformed |

Le code n'appelle jamais `WindowsVhfTransport` ou `DeviceIoControl`,
ne charge pas le `.sys`, n'eleve pas l'interface Tauri, n'installe aucun
service et ne modifie ni Bluetooth, ni Steam Input, ni la manette physique.

## Validation CI (ne demarre pas le serveur)

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc --bin padbridge-broker-preview
```

## A faire avant tout deploiement ou test interactif VM

1. Implementer l'entree SCM, la gestion STOP et le nettoyage deterministe ;
   actuellement `ConnectNamedPipe` / `ReadFile` sont bloquants, sans
   delai ni mecanisme d'annulation. Ce n'est PAS un service deployable.
2. Confirmer l'identite du client **par son jeton Windows** (impersonation)
   en plus de l'ACL, en particulier les sessions et les comptes multiples.
3. Extraire le SID autorise d'une **configuration installee de facon
   privilegiee et protegee** ; l'argument CLI du POC sert uniquement
   a une experience de developpement et n'est pas une autorisation
   d'installation sure.
4. Ajouter limitation de debit, limite de connexions, duree maximale de
   requete, arret des sessions, bail d'entree et journalisation minimale.
5. Auditer l'interface noyau (watchdog/PnP/D0/surprise remove), puis
   preparer un INF et signer pour la VM isolee avec point de controle.
6. Seulement apres ces validations, etudier le raccordement eventuel du
   transport kernel a un service SCM de moindre privilege et un client
   PadBridge non eleve ; l'interface utilisateur reste sur `NullBackend`.
7. XInput/XUSB reste **hors du perimetre** de ce gamepad HID generique.

Aucune installation sur la machine hote, et pas de fusion de la PR brouillon
sans validation explicite de l'utilisateur.
