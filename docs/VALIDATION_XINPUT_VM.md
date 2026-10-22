# POC B — preuve de compatibilité XInput sous Windows 11

## État du projet et hypothèse à vérifier

Le pilote PadBridge KMDF/VHF expérimental déclare un **gamepad HID générique**. Cela ne démontre aucune compatibilité XInput/XUSB. Une interface HID visible dans le Gestionnaire de périphériques ou dans joy.cpl ne suffit pas à dire que des jeux XInput, notamment EA FC, l'accepteront.

L'outil `padbridge-xinput-probe` réalise un instantané **en lecture seule** de `XInputGetState` dans XInput 1.4. Il interroge uniquement les emplacements 0, 1, 2, 3, et renvoie JSON avec `connected`, `disconnected` ou `error`. Les états actifs comportent le numéro de paquet, le masque de boutons, les gâchettes et les quatre axes. Aucun chemin HID, numéro de série ou identifiant personnel n'est recueilli.

## Construction (Windows, sans interface graphique, sans pilote)

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features xinput-probe-poc
cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --features xinput-probe-poc --bin padbridge-xinput-probe -- --snapshot
```

Ces commandes **ne servent pas à installer un pilote**. Elles ne lisent que l'état XInput Windows. L'outil est isolé derrière `xinput-probe-poc`, n'est pas exposé dans les menus Tauri, ne lance aucun service et ne modifie pas les manettes.

## Capture de référence XInput, sans pilote

Dans la VM **Hyper-V Windows 11** avec un point de contrôle déjà créé depuis l'hôte, compiler une fois la commande de lecture XInput (aucune installation) :

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features xinput-probe-poc --bin padbridge-xinput-probe
.scriptscollect_vm_xinput_baseline.ps1 -Stage before
```

Le script refuse de fonctionner sur une machine qui n'est pas identifiée comme invitée Hyper-V Microsoft et enregistre uniquement `vm-evidence/before.json`. Il ne crée PAS de point de contrôle, ne charge PAS de pilote et ne modifie aucun service.

Lors d'une phase ultérieure **autorisée et isolée** (après sélection d'un backend réellement XInput, installation/signature revue et contrôles de sécurité), capturer :

```powershell
.scriptscollect_vm_xinput_baseline.ps1 -Stage after-idle
# Déclencher ensuite une action de test dans le backend validé de la VM.
.scriptscollect_vm_xinput_baseline.ps1 -Stage after-activity
python scripts/compare_xinput_snapshots.py --before vm-evidence/before.json --after-idle vm-evidence/after-idle.json --after-activity vm-evidence/after-activity.json
```

Le verdict `candidate_input_change_unattributed` n'est qu'un **candidat** : il faut une preuve PnP indépendante de l'identité du contrôleur. Les trois captures ne prouvent jamais, seules, la compatibilité PadBridge avec XInput.

Document de décision et limites : [ARCHITECTURE_XINPUT_DECISION.md](ARCHITECTURE_XINPUT_DECISION.md).

## Plan de validation VM Hyper-V, avant toute mise en production

**Prérequis obligatoires :** VM Windows 11 démarrée et isolée, point de contrôle récent, aucun périphérique réel en cours de test, processus de réinitialisation connu. Pas de désactivation durable de Secure Boot ou des protections noyau de la machine principale. Ne pas installer le pilote PadBridge non signé dans le système hôte.

1. **Référence sans pilote :** utiliser `--snapshot` dans la VM propre. Noter les quatre états ; l'absence d'une manette XInput devrait donner quatre emplacements `disconnected`. Si un appareil XInput existe déjà, ne pas lui attribuer le statut PadBridge.
2. **Revue noyau préalable :** examiner PnP/D0, VhfStart/VhfDelete, le watchdog 800 ms, les callbacks de retrait, l'exclusivité, les ACL et le comportement après perte du broker. L'absence actuelle de paquet INF et de signature reste un blocage : ne pas procéder à une installation improvisée.
3. **Validation HID distincte (future) :** après création d'un paquet d'installation et validation/sécurisation du pilote pour la VM, vérifier que le nouveau périphérique HID VHF s'énumère correctement. **Ne pas déclarer XInput réussi à cette étape.**
4. **Preuve XInput (future) :** comparer les quatre résultats XInput avant/après. Il faut un emplacement XInput nouvellement détecté **et attribué sans ambiguïté au périphérique testé**, puis des changements de `packetNumber`, boutons, axes et gâchettes en réponse à des rapports de test autorisés.
5. **Déconnexion et sécurité (future) :** libérer les boutons, couper le broker, simuler la perte de trames, le retrait surprise et la mise en veille/reprise. Vérifier absence de contrôle fantôme et neutralisation, dans les limites du watchdog 800 ms prévu par le code.
6. **Décision de conception :** si le pilote VHF HID ne remplit pas les conditions XInput, documenter explicitement l'échec du critère et choisir/reconcevoir une architecture compatible avant de promettre une manette Xbox virtuelle.

## Critères d'acceptation

| Critère | État |
|---|---|
| ABI `XINPUT_STATE` vérifiée par tests Rust | Test automatisé inclus |
| Exécutable de diagnostic XInput 1.4 compile en CI Windows | Vérifié par CI avant acceptation |
| API XInput accessible sur runner Windows propre | Test de lecture seule en CI |
| Appareil VHF HID reconnu dans VM | Non validé |
| PadBridge génère une nouvelle manette reconnue par `XInputGetState` | Non validé |
| Conversion des entrées physiques et neutralisation dans la VM | Non validé |
| Pilote signé, installation/désinstallation propre | Non implémenté |

**Attention :** un résultat connecté ne prouve jamais à lui seul qu'il appartient à PadBridge. L'outil ne tente pas de l'attribuer. Il ne contrôle pas le pilote. Il ne fait aucun test de vibration ni d'écriture HID.

Source API Microsoft : https://learn.microsoft.com/en-us/windows/win32/api/xinput/nf-xinput-xinputgetstate
