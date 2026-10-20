# Service broker PadBridge — prototype IPC local V1, inactif par defaut

## Situation

Cette etape contient le **protocole IPC**, la preview du pipe Windows
et un **squelette de cycle de vie SCM inerte**. Aucun service n'est enregistre
ou demarre sur le PC/VM, et le programme SCM n'active pas le serveur IPC.

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
  Apres lecture du message, la preview **emprunte l'identite du client**
  via `ImpersonateNamedPipeClient`, consulte son `TokenUser` et compare le SID
  reel a celui autorise. Tout echec renvoie `Unauthorized` ;
  `RevertToSelf` est impose avant toute operation privilegiee.
  Les appels connect/read/write sont en mode **OVERLAPPED** avec un delai
  de 2 500 ms chacun et annulation `CancelIoEx` suivie d'un drainage avant
  liberation des buffers.
- `src-tauri/src/bin/padbridge-broker-preview.rs` : programme de
  demonstration *one-shot* derriere l'option de compilation
  `broker-service-poc`. Refuse tout demarrage sans parametre explicite.
  Il repond a **un seul** message et se termine sans toucher au pilote.
- `src-tauri/src/virtual_pad/broker_auth_windows.rs` : verification du SID
  du client reel, avec restauration imperative du jeton du serveur.
- `broker_scm_windows.rs` + `src/bin/padbridge-broker-scm.rs` :
  entree `StartServiceCtrlDispatcherW`, enregistrement du handler et des
  transitions `START_PENDING` → `RUNNING` → `STOP_PENDING` → `STOPPED`.
  Un event STOP reveille la boucle **inerte**. Aucun pipe, IPC ni pilote
  n'est demarre par cette version. Aucune installation SCM.

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
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc --bin padbridge-broker-scm
```

## A faire avant tout deploiement ou test interactif VM

1. **Ne pas raccorder le listener pipe au SCM avant audit** : les
   composants SCM et IPC sont actuellement independants. Il faut relier
   les ordres STOP a l'annulation de l'I/O en cours, definir le cycle de
   vie des clients et reporter les erreurs dans les statuts SCM.
2. **Tester vraiment les controles Windows en VM** : refus des jetons non
   autorises et des clients distants, ACL de l'instance, jetons de session,
   expiration des 2,5 secondes, erreurs de cancellation et usurpation SID.
   Les tests CI actuels ne font pas ces tests d'integration.
3. Extraire le SID autorise d'une **configuration installee de facon
   privilegiee et protegee** ; l'argument CLI du POC sert uniquement
   a une experience de developpement et n'est pas une autorisation
   d'installation sure. Ne pas autoriser le SCM a lire un SID arbitraire.
4. Ajouter limitation de debit, limite de connexions, renouvellement des
   baux et journalisation minimale ; maintenir l'injection desactivee.
5. Auditer l'interface noyau (watchdog/PnP/D0/surprise remove), puis
   preparer un INF et signer pour la VM isolee avec point de controle.
6. Seulement apres ces validations, etudier le raccordement eventuel du
   transport kernel a un service SCM de moindre privilege et un client
   PadBridge non eleve ; l'interface utilisateur reste sur `NullBackend`.
7. XInput/XUSB reste **hors du perimetre** de ce gamepad HID generique.

Aucune installation sur la machine hote, et pas de fusion de la PR brouillon
sans validation explicite de l'utilisateur.
