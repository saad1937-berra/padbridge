# T14 — protection des sorties HID et tests de régression

## Périmètre

T14 reste suspendu après des essais de vibration Bluetooth non concluants.
Les protections ajoutées dans Rust complètent le bouton désactivé dans React :

- Commandes Tauri de vibration refusées avant l'accès matériel.
- RumbleRegistry::pulse et RumbleRegistry::stop refusent de placer une commande HID en file.
- Le lecteur HID vérifie la politique avant toute écriture de vibration, y compris au nettoyage.

Le diagnostic et la lecture des boutons restent inchangés. Aucun pilote n'est installé.

## Tests

- t14_output_is_blocked_without_queueing_commands : aucune écriture mise en file.
- ds4_bluetooth_press_release_and_reconnect_reports : rapports Bluetooth DS4 synthétiques, pression puis relâchement de Croix, puis rapport neutre après reconnexion simulée.

Ces tests ne prouvent PAS la résolution de l'incident intermittent observé dans joy.cpl et n'impliquent aucune manette réelle.

## Règles

Ne pas réactiver les vibrations sur le PC principal. Tout essai noyau doit rester en VM Hyper-V isolée avec point de contrôle. Ne pas fusionner avec main avant validation Windows.

## CI

cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-scm-poc
npm run build
