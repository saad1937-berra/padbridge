# Contrat binaire VHF — vérification statique automatique

## Pourquoi ce garde-fou existe

Le code Rust construit un rapport HID virtuel de 10 octets et le noyau déclare un descripteur HID ainsi qu'une structure C correspondante. Une divergence silencieuse (nombre de boutons, offsets, ordre, axes, durée du watchdog) peut provoquer une perte de commandes ou un comportement dangereux lors de futurs tests en VM.

Cette étape ajoute `scripts/verify_vhf_contract.py`, indépendant des compilateurs WDK/Rust. Il lit exclusivement trois fichiers source :

- `driver/poc-vhf/PadBridgeVhf.c` (descripteur HID générique)
- `driver/poc-vhf/PadBridgeVhf.h` (structure C packée et constantes)
- `src-tauri/src/virtual_pad/vhf_protocol.rs` (constantes et ordre des boutons Rust)

Le parseur HID vérifie les groupes Input, leurs tailles en bits, les pages et usages, le type des champs, les valeurs logiques, les offsets et le total exact du rapport. Il vérifie aussi que la structure C packée correspond aux 10 octets prévus, que les 11 boutons ont le même ordre des deux côtés, que l'IOCTL est identique et que les constantes de watchdog restent sur la politique attendue.

## Exécution locale (aucun pilote, aucune élévation)

```powershell
python scripts/verify_vhf_contract.py --self-test
```

L'option `--self-test` injecte trois divergences fictives **uniquement en mémoire** (nombre de boutons HID, valeur neutre du D-pad, longueur du rapport Rust). La vérification doit refuser chacune d'elles.

Les deux workflows GitHub `PadBridge Windows CI` et `PadBridge Driver WDK (build only)` exécutent cette vérification avant les étapes de compilation. Le script utilise exclusivement la bibliothèque standard Python et ne possède aucun accès Windows, HID, Bluetooth ou kernel.

## Limites explicites

- Une validation de sources **ne démontre ni la conformité complète du pilote KMDF, ni la réussite de VhfStart, ni la stabilité du watchdog réel**.
- Le descripteur testé correspond à un **périphérique HID de jeu générique**, pas à un périphérique XUSB/XInput. La conversion XInput reste **non démontrée**.
- Aucun INF ni pilote signé/installable n'est créé par ce contrôle.
- Ne jamais charger le `.sys` expérimental sur l'hôte principal. Tout essai ultérieur de chargement noyau nécessitera un paquet installable audité et une VM Windows isolée avec point de contrôle.

## Après la validation

Le prochain jalon est l'audit fonctionnel et sécurité de la durée de vie KMDF/VHF (entrées/sorties D0, destruction VHF, timer et callbacks, ACL, collisions d'arrêt), suivi d'un protocole de VM. L'outil XInput indépendant de la PR précédente devra être utilisé pour distinguer une simple émulation HID d'une véritable compatibilité XInput.
