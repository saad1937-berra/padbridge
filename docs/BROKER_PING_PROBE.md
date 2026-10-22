# POC B — test de liaison au broker local, sans injection

## Objectif

Valider que le protocole IPC v1 est interprété correctement côté client et que le pipe nommé local peut répondre à une requête de santé PING.

Cette étape ne crée **aucun gamepad virtuel**, ne rend **pas XInput disponible**, n'ouvre **aucun pilote KMDF/VHF**, n'installe **aucun service** et ne touche **ni au Bluetooth ni aux commandes HID**.

## Fichiers

- `virtual_pad/broker_ipc.rs` : validation stricte des réponses v1 (longueur, signature, version, octets réservés, codes de résultat).
- `virtual_pad/broker_client_windows.rs` : client limité à PING, chemin de pipe fixe, droits minimaux READ_DATA/WRITE_DATA, I/O overlapped bornées à 1,5 seconde avec annulation et drainage.
- `src/bin/padbridge-broker-probe.rs` : exécutable opt-in `--ping-only`, uniquement dans un environnement de test isolé.
- `virtual_pad/broker_pipe_windows.rs` : tests Windows synchronisés pour éviter qu'ils tentent simultanément de créer l'unique pipe de POC.

## Vérification automatique

Le job Windows CI exécute les tests Rust sous `--features broker-service-poc` (y compris l'ouverture d'un pipe temporaire et son PING authentifié) et vérifie la compilation du binaire client. Le test récupère le SID réel de son propre processus Windows, sans registre HKLM ni intervention système.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc --bin padbridge-broker-probe
```

## Limites

- Client désactivé dans les builds Tauri normaux ; pas d'intégration `selected_backend.rs`.
- Commandes `SUBMIT` et `NEUTRAL` toujours refusées par le serveur ; le nouveau client ne peut pas les émettre.
- Aucune preuve de compatibilité XInput, de signature de pilote, de protection de service SCM en production ni de résilience du matériel réel.
- Ne pas installer ni lancer le broker et le pilote expérimentaux sur le PC principal. Poursuivre les validations uniquement sur runner jetable ou VM Hyper-V isolée avec snapshot.
