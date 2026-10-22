# SCM preview : intégration sûre du worker autonome (sans pilote)

## Objectif

Relier le prototype d'ordonnanceur de la PR #8 à la durée de vie du **service Windows expérimental** (SCM), tout en **interdisant techniquement** l'envoi de commandes à une manette ou à un pilote. La branche reste un POC de validation interne.

## Démarrage

Le service n'est jamais installé automatiquement et exige toujours une configuration SID valide déjà approvisionnée dans HKLM par un administrateur. L'accès au pipe local reste limité par sa DACL et une vérification du jeton de l'expéditeur.

Après avoir créé le pipe, le code démarre `PreviewRuntime`, qui crée un `BrokerWorker` avec `NoDriverSink`. Ce transport reçoit uniquement le **rapport HID neutre** et l'enregistre dans un compteur en mémoire. Il ne contient ni `CreateFile` sur une interface périphérique, ni `DeviceIoControl`, ni handle de pilote. Si la création du worker échoue, le service ne se déclare jamais `RUNNING`.

## Sécurité des entrées

- L'IPC accepte seulement la commande de diagnostic `PING` et la fermeture du protocole, selon la politique `handle_dry_run` existante.
- Les commandes `SUBMIT` et `NEUTRAL` restent désactivées pour **tous** les utilisateurs (y compris les administrateurs).
- `PreviewRuntime` ne rend **aucune méthode d'ajout d'entrées** accessible à l'IPC. Aucun rapport actif n'est soumis au worker.
- Le transport mémoire vérifie encore chaque trame ; tout rapport différent du neutre retourne `PermissionDenied`.
- Le backend Tauri normal reste `NullBackend`. Les vibrations T14 et l'installation du pilote ne sont pas activées par cette intégration.

## Arrêt / échec

Lors du contrôle SCM STOP, l'I/O pipe en attente est annulée par le mécanisme existant, la boucle de service se ferme, puis `runtime.shutdown()` ferme son canal et attend le thread. Le worker termine par un rapport neutre **en mémoire seulement**. Les erreurs d'arrêt du worker sont répercutées via le code d'arrêt SCM si aucune autre erreur ne doit être conservée.

Le service ne peut pas garantir le délai d'arrêt d'un futur transport noyau tant que ses opérations bloquantes ne disposent pas de deadlines et d'une annulation vérifiée. Le transport actuel est uniquement en mémoire et synchrone.

## Tests CI

Les tests Rust sur Windows vérifient que le worker crée une trame neutre au démarrage, qu'aucun heartbeat supplémentaire n'est généré à l'état neutre, que le transport mémoire refuse une commande active, et que même une trame `SUBMIT` valide est refusée par la politique du pipe. L'étape `broker-scm-poc` vérifie le fonctionnement des composants Windows sans installation de service.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc broker_preview_runtime
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc --bin padbridge-broker-scm
```

## Jalons restants

1. Tester un vrai cycle de vie SCM STOP / START **uniquement dans une VM isolée**, avec point de contrôle, logs et procédure de restauration.
2. Valider qu'un futur transport DeviceIoControl possède une annulation et une limite de temps fiables et que le système neutralise l'état en cas de panne.
3. Revoir la sécurité du chemin de soumission des entrées, la signature/installation INF, le retrait surprise PnP et les états d'alimentation D0.
4. Démontrer une compatibilité **XInput réelle**, qui n'est pas garantie par le périphérique VHF HID générique actuel.

**Aucun `.sys` n'est installé ou exécuté sur l'hôte principal. Le POC SCM reste non déployable en production.**
