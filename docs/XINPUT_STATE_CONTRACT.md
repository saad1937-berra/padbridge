# XInput — contrat d'état indépendant du pilote (POC C)

## Objet

Préparer l'interface d'un futur backend Xbox sans ajouter de dépendance ni installer de pilote. Le moteur existant convertit déjà une manette PlayStation vers `OutputState` ; ce lot vérifie que le résultat final possède les boutons, gâchettes et axes attendus par la structure publique **`XINPUT_GAMEPAD`**.

**Attention : la structure XINPUT_GAMEPAD n'est PAS un paquet transport XUSB, HID, ni une preuve que Windows ou un jeu détectera une manette Xbox.** Les 12 octets ici servent exclusivement de contrat/fixture de test pour de futurs adaptateurs.

## Livrables

- `src-tauri/src/virtual_pad/backend.rs` : `#[repr(C)]` explicite sur `XInputReport`, de même taille et alignement que `XINPUT_GAMEPAD` (12 octets, alignement 2).
- `src-tauri/src/virtual_pad/xinput_state_contract.rs` : validation des six nombres flottants normalisés (sans NaN/infini/débordement), transformation vers les masques XInput existants et fixture little-endian explicite.
- `src-tauri/src/virtual_pad/xinput_state_loopback.rs` : backend **exclusivement en tests Rust** enregistrant l'état Xbox en mémoire, sans driver et sans accès HID.

## Tests

- Les 14 boutons courants Xbox (boutons de face, gâchettes d'épaule, Start/Back, sticks cliquables, D-pad) ont les masques attendus.
- La sortie neutre est 12 octets à zéro ; les sticks signés, les gâchettes et les boutons respectent leur ordre de champs et leurs plages.
- NaN, infini et dépassements hors plage sont refusés avant un futur passage dans un processus privilégié.
- Les rapports DS4 **synthétiques** USB et Bluetooth traversent le véritable moteur, leurs profils et leurs zones mortes, et aboutissent à des états `XINPUT_GAMEPAD` en mémoire.
- Le mapping Croix→Y, la déconnexion/reconnexion et le retour au mode natif en cas d'échec de soumission sont vérifiés.

## Recherche d'un véritable backend XInput

1. ViGEmBus est officiellement **archivé depuis 2023** ; nous ne devrions pas l'adopter comme nouveau backend par défaut. Référence : https://docs.nefarius.at/projects/ViGEm/End-of-Life/.
2. LizardByte libvirtualhid a une implémentation Windows UMDF2 avec broker et ses **documents de développement récents** mentionnent un compagnon XUSB pour le profil Xbox 360. Référence : https://github.com/LizardByte/libvirtualhid/blob/master/docs/windows-driver.md.
3. Cette offre impose **une licence machine Windows** pour les gamepads pilotés. L'interface cliente est publique, mais la licence du package driver, les droits de redistribution, le cycle de maintenance et l'attribution XInput en conditions réelles demandent un audit séparé. Référence : https://app.lizardbyte.dev/libvirtualhid/virtual-hid-driver/.
4. Certaines pages publiques de présentation LizardByte décrivent des profils Xbox HID sans compatibilité Xbox 360 sur le chemin UMDF/VHF seul. Il faudra vérifier **la version exacte du package**, son éventuel composant XUSB compagnon et sa signature avant toute intégration, pas supposer une prise en charge depuis le nom d'un profil.

**Aucune dépendance libvirtualhid, ViGEm ou autre pilote tiers n'est ajoutée dans cette branche.**

## Verrouillage produit

`selected_backend.rs` continue de créer `NullBackend`. Le passage de données DS4 à des états `XINPUT_GAMEPAD` réussis en simulation ne signifie pas que la virtualisation Xbox fonctionne. Avant de rendre ce mode disponible : sélectionner et auditer un pilote réellement XUSB, valider la licence, créer un transport authentifié à opérations bornées, établir l'identité PnP et des tests XInput dans un environnement isolé.

Commandes CI :

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features xinput_state_contract
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features xinput_state_loopback
```
