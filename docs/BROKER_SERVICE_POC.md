# PadBridge — SCM + IPC local, POC v2 (non deploye)

## Etat et limites

Le code sait **relier le controle STOP du SCM a une boucle de serveur
Windows sur pipe nomme** et interrompre un Connect/Read/Write overlapped
en attente. Il ne constitue toujours **PAS** un service de production :
aucun installateur ni enregistrement SCM, configuration administrative
non provisionnee et aucun essai reel dans une VM equipee du pilote.

La VM du developpeur et le Windows hote n'ont subi aucune modification.
`selected_backend.rs` reste sur `NullBackend` ; ni les manettes physiques,
ni le Bluetooth, ni le pilote KMDF n'ont ete touches. Le gamepad HID
generique n'est pas XInput.

## Sources et architecture

- `virtual_pad/broker_ipc.rs` : trames v1 de 18 octets,
  reponses fixes de 8 octets. `PING` et `CLOSE` autorises, `SUBMIT`
  et `NEUTRAL` **toujours refuses** (status `Disabled`), meme valides.
- `virtual_pad/broker_pipe_windows.rs` : un pipe nomme local
  `\\\\.\\pipe\\PadBridge.Broker.POC.v1` avec DACL protegee
  SYSTEM / Administrateurs / SID utilisateur defini par configuration ;
  pas de `FILE_CREATE_PIPE_INSTANCE` pour cet utilisateur, pas de
  connexions distantes ; une instance.
- `virtual_pad/broker_auth_windows.rs` : a chaque message recu,
  `ImpersonateNamedPipeClient`, ouverture du **jeton de thread**
  et comparaison du `TokenUser` reel avec le SID de configuration.
  En cas d'echec, statut `Unauthorized`. `RevertToSelf` obligatoire.
- `virtual_pad/broker_config_windows.rs` : lecture seule de
  `HKLM\\SOFTWARE\\PadBridge\\BrokerPreview\\AuthorizedClientSid`
  au format REG_SZ. Pas de valeur de secours ni de SID de CLI : le
  processus **refuse** la mise en service du listener si cette valeur
  est absente, malformee ou inaccessible.
- `virtual_pad/broker_scm_windows.rs` : `StartServiceCtrlDispatcherW`,
  `RegisterServiceCtrlHandlerExW`, `START_PENDING`, `RUNNING`,
  `STOP_PENDING`, `STOPPED`. Le handler STOP ne fait qu'activer
  une variable atomique. Un poll de 50 ms dans les I/O overlapped
  interrompt l'attente ; `CancelIoEx` et `GetOverlappedResult`
  attendent l'achevement de l'annulation avant de liberer les buffers.
  Le delai maximal par operation reste 2,5 s.
- `src/bin/padbridge-broker-scm.rs` : point d'entree compile avec
  `broker-scm-poc` seulement. Ne lance pas de service de lui-meme ;
  il exige le contexte d'invocation SCM. Aucun appel a
  `WindowsVhfTransport`, `DeviceIoControl`, aucun `.sys`.

Le programme `padbridge-broker-preview.rs` reste une preview autonome
une requete seulement. Son argument SID de test **n'est pas** une
configuration de confiance et n'est pas utilise par le code SCM.

## Important : provisioning manquant, aucun guide d'installation

Un futur installateur privilegie devra creer la cle machine
`HKLM\\SOFTWARE\\PadBridge\\BrokerPreview` avec une ACL d'ecriture
**reservee aux administrateurs de confiance/SYSTEM** et une valeur SID
canonique ; le programme ne cree pas la cle lui-meme. Les ACL, la
separation des identites de service et le modele multi-session doivent
etre audites avant de declarer cette protection sure. Ne pas injecter
une valeur dans la registry du PC hote pour essayer ce POC.

Le SCM peut mettre le listener en etat RUNNING **uniquement lorsqu'il
dispose de cette configuration machine** et s'il parvient a creer le
pipe. L'application Tauri et le pilote restent deconnectes du broker.

## CI de compilation et de tests (aucune installation)

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc --bin padbridge-broker-scm
```

Les tests couvrent le protocole, les SID invalides et l'interruption d'un
`ConnectNamedPipe` en attente sur le runner Windows. Aucune execution
en SCM ni installation du pilote n'est effectuee en CI.

## Avant toute mise en service reelle

1. Essai en **VM Windows isolee avec point de controle**, d'abord IPC
   seulement : installation SCM revue, configuration HKLM protege,
   controles d'identite utilisateur, comptage de handles et STOP
   au milieu de Connect/Read/Write.
2. Tester les races STOP/connexion, erreurs `CancelIoEx` et nettoyage,
   multi-session, usurpation de clients, controle de debit, resilience.
3. Remplacer la configuration provisoire par un provisioning signe /
   privilegie et definir une identite de service de moindre privilege.
4. Auditer le pilote noyau, le watchdog et PnP/D0/surprise-remove,
   preparer l'INF et la signature pour VM uniquement.
5. Garder toutes les commandes vers le pilote desactivees tant que
   cet audit et les tests noyau ne sont pas termines.
6. Etudier separement la compatibilite XInput/XUSB et la distribution.

**Aucun changement sur `main`, aucun deploiement hote, et aucune
fusion de la PR sans approbation explicite.**
