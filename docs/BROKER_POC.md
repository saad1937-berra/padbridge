# POC B — broker Rust (code preparatoire, non deploye)

## Etat (20/10/2026)

Le coeur du broker et le transport Windows sont dans les sources **mais
ne constituent PAS un service Windows installe**. Aucun chemin dans la
fenetre Tauri ne declenche l'ouverture d'un appareil. Le backend de PadBridge
reste `NullBackend`, et le pilote KMDF reste **non signe/non installe**.

Ce POC n'emule **pas** XInput, n'installe pas de pilote et ne touche pas aux
manettes physiques, Bluetooth, Steam Input, vibrations ni au systeme hote.

## Architecture actuelle

- `src-tauri/src/virtual_pad/vhf_protocol.rs` : rapport HID de 10 octets
  `[id,boutons_lo,boutons_hi,hat,LX,LY,RX,RY,LT,RT]`, validation, IOCTL
  `0x0022A000`.
- `vhf_broker.rs` : `BrokerSession<T: ReportTransport>`. Demarre par un
  rapport neutre, valide les rapports, tente un rapport neutre en cas d'erreur
  et lors de `close()`/`Drop`. `heartbeat()` renvoie les etats actifs
  au plus toutes les 200 ms **si l'appelant l'invoque periodiquement**.
  Le cache actif n'est maintenu que si une entree fraiche a ete soumise
  pendant les **600 ms precedentes**. Le heartbeat ne renouvelle jamais
  ce bail ; si le flux physique se bloque, le broker tente une trame
  neutre puis ferme la session en erreur TimedOut. Le watchdog noyau
  de 800 ms reste le secours si l'ordonnanceur se bloque ou si l'ecriture
  neutre echoue.
- `vhf_broker_windows.rs` : transport optionnel Windows. Enumere le GUID
  prive `E23BD620-6064-4879-9C10-1A5D422BCC21` par SetupAPI, refuse
  une enumeration ambigue, demande une ouverture exclusive et envoie les
  rapports valides avec `DeviceIoControl`. L'ACL du pilote determine
  l'autorisation. Pas de creation d'appareil, pas de service, pas d'elevation.
- `selected_backend.rs` : toujours `NullBackend`, **jamais le broker**.

## Validation de code seulement

Sur le CI Windows (aucun pilote installe) :

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-poc
cargo check --manifest-path src-tauri/Cargo.toml --features desktop,broker-poc
```

Le transport Windows est cache derriere la feature `broker-poc`, absente
des features par defaut. Meme compile, il n'est appele par aucun chemin
d'execution de l'application. Les tests sont simules et ne valident ni les
appels SetupAPI effectifs ni `DeviceIoControl` avec un pilote installe.

## Conditions AVANT tout service/installation

1. Auditer le chemin de l'interface KMDF, les transitions PnP/D0, le
   watchdog, les collisions d'arret, le comportement de `VhfDelete`
   et la gestion d'erreurs en noyau.
2. Definir un service Windows minimal dont **seule l'identite de service**
   peut ouvrir le pilote. Etudier une ACL propre au service, pas une
   augmentation des droits de la fenetre Tauri.
3. Concevoir un IPC **local, authentifie et a ACL stricte**, avec autorisation
   par session utilisateur et controles de debit. Ne pas exposer l'IOCTL brut
   ni un chemin de peripherique choisi par un client a travers l'IPC.
4. Gerer l'exclusivite, la disparition du peripherique, la perte des entrees,
   l'arret du service et les expirations de bail. Le code du broker requiert
   qu'un ordonnanceur appelle `heartbeat()` periodiquement ; cet
   ordonnanceur n'est pas implemente. Le bail de 600 ms protege un
   heartbeat uniquement lorsque cet ordonnanceur est actif ; il ne
   garantit ni l'execution du service ni l'absence de blocage noyau.
5. Preparer une installation INF et une signature **uniquement pour
   la VM de test**, avec point de controle et outils de debogage noyau.
   Ne pas desactiver les protections du PC principal.
6. Valider dans la VM : connexion, plusieurs handles refuses, perte
   du client, fermeture, bouton maintenu plus de 800ms, charge, veille/reprise,
   surprise-remove et restauration de l'etat neutre.

Ne pas fusionner la PR brouillon ou annoncer la compatibilite XInput avant
ces validations.
