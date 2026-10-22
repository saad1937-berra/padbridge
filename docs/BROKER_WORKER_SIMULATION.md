# Broker POC — worker autonome de battement (simulation)

## Ce qui est livré

Le module Rust `src-tauri/src/virtual_pad/broker_worker.rs` exécute une session `BrokerSession` sur un thread dédié. Il est compilé uniquement en tests ou sous la feature **explicitement optionnelle** `broker-service-poc` ; aucun chemin Tauri ni backend sélectionné ne le lancent. Le service SCM **expérimental** peut désormais le posséder uniquement avec un transport de rapports neutres en mémoire (voir `docs/SCM_DRYRUN_WORKER_RUNTIME.md`) ; aucun pilote ne reçoit de rapport.

Un canal borné (`sync_channel`, 8 rapports maximum) transporte des entrées de manette **déjà validées** vers ce worker. L'API `queue_fresh` confirme uniquement la mise en file, pas l'écriture effective sur un pilote. Si la file est saturée, un envoi échoue sans blocage en `WouldBlock` et la session existante n'est pas prolongée par cet événement rejeté.

La boucle vérifie le heartbeat toutes les **50 ms**. Elle ne renvoie un rapport actif que si le dernier paquet **fraîchement reçu** date de moins de **600 ms**. Le watchdog du pilote garde son délai de **800 ms** et reste indépendant.

## Correction de la date des entrées

Chaque paquet est horodaté **avant sa mise en file** (`Instant::now` du producteur, origine contrôlée côté broker). Le nouveau `send_observed` conserve cet instant, au lieu de considérer que le paquet est frais à la date du traitement. Une entrée périmée ou dans le désordre est rejetée et déclenche une tentative de neutralisation. Sans cela, un worker momentanément ralenti pourrait ressusciter une touche ancienne.

## Fermeture

À la fermeture du canal, le worker émet un rapport neutre best effort via `BrokerSession::close` et rend le résultat. Le destructeur libère le canal et attend aussi la fin du thread. **Limite :** cela peut bloquer si le transport lui-même ne respecte aucun délai ; un futur transport Windows devra garantir des I/O bornées et annulables avant tout emploi en service.

## Scénarios CI

- Envoi d'un appui simulé, battement automatique sans appel manuel et libération à l'arrêt.
- Fermeture du producteur, rapport neutre final.
- Absence d'entrée fraîche pendant 600 ms, session fermée et erreur `TimedOut`.
- Aucun bouton actif : aucun rapport de battement superflu.
- Conserver l'heure de réception d'un paquet malgré son traitement différé ; rejeter les paquets expirés.

Exécution Windows :

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc
```

## Limites de sécurité et prochaines étapes

Il s'agit d'un POC **entièrement simulé** : le lecteur Bluetooth physique n'alimente pas ce canal, les services SCM restent dry-run, les commandes d'injection IPC restent refusées et `selected_backend.rs` utilise toujours `NullBackend`. Aucun pilote KMDF/VHF n'est installé ni exécuté. Ce module ne prouve pas XInput/XUSB.

Avant une intégration réelle : vérifier l'identité et l'autorisation des entrées IPC, traiter les pertes de périphérique, instrumenter la latence de réception, ajouter l'annulation Windows pour les transports bloqués, valider les scénarios PnP/D0 et la neutralisation en VM, et obtenir un pilote installable et signé. Ne jamais tester le `.sys` expérimental sur la machine Windows principale.
