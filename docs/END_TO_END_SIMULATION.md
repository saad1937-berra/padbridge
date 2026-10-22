# POC B — tests de bout en bout sans pilote

Cette phase relie **en simulation uniquement** les composants qui devront alimenter le futur pilote VHF :

1. Rapport HID DualShock 4 synthétique (USB 0x01 ou Bluetooth 0x11).
2. Décodage et normalisation par le vrai moteur Rust.
3. Application du profil de touches et des zones mortes.
4. Session de sortie via le vrai BrokerSession Rust.
5. Sérialisation VHF de 10 octets vers une mémoire de test, sans aucun DeviceIoControl.

## Couverture

- `usb_hid_to_virtual_wire_and_disconnect` : touche Croix → A, L1 → LB, stick et L2, puis neutralisation à la perte de périphérique.
- `bluetooth_reconnect_never_replays_old_buttons` : anciennes touches libérées, nouvelle session neutre, aucun appui conservé après reconnexion simulée.
- `profile_change_updates_wire_bytes_not_only_preview` : changement de mapping Croix → Y transmis jusqu'au rapport VHF simulé.
- `broken_transport_neutralizes_then_falls_back_to_native` : panne du transport simulé, tentative neutre et retour au mode natif.

Le module `src-tauri/src/virtual_pad/loopback_poc.rs` est compilé uniquement avec `#[cfg(test)]`. Il n'est disponible ni dans le Tauri normal, ni via un paramètre ou menu. `selected_backend.rs` demeure sur `NullBackend`.

## Validation automatique

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features loopback_poc
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
```

La CI Windows vérifie aussi les builds Tauri et les prototypes IPC. Ces tests ne constituent pas une preuve XInput : un périphérique VHF HID générique n'est pas forcément XUSB et la méthode `XInputGetState` n'a toujours pas été validée.

## Prochain jalon technique

Préparer une machine de test Windows isolée avec point de contrôle Hyper-V et revue du code KMDF/VHF (PnP, retrait, watchdog, verrouillage, audit ACL). Ne jamais installer le pilote non signé sur le PC principal. Avant toute sortie vers les jeux, démontrer une compatibilité XInput réelle et un modèle d'installation/signature adapté.
