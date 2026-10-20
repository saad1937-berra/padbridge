# POC B — pilote virtuel PadBridge (KMDF + VHF)

**Etat : prototype experimental compile avec WDK x64 dans GitHub Actions (20/10/2026), mais NON signe, NON installe et NON teste en execution. Ne PAS l'installer sur le Windows 11 principal.**

Ce repertoire introduit une premiere base de pilote noyau **maison**, ecrite en C avec
KMDF et **Virtual HID Framework (VHF)** de Microsoft. Il existe seulement pour
developper, auditer puis essayer la creation d'un **gamepad HID generique**
sur une machine virtuelle Windows de test.

**Ce n'est PAS une manette Xbox ni un backend XInput fonctionnel.** Un jeu
qui exige XInput ne sera pas rendu compatible par le seul fait d'enumerer
un gamepad HID. Microsoft explique que la pile XUSB expose une interface
noyau consommee par XInput, distincte de l'interface HID/DirectInput.

## Arborescence POC

- `PadBridgeVhf.h` : format binaire ABI de 10 octets + IOCTL prive.
- `PadBridgeVhf.c` : point d'entree KMDF, interface d'appareil, validation
  stricte des rapports, descripteur HID et appels VHF.
- `PadBridgeVhf.vcxproj` : projet Visual Studio **KMDF x64** autonome (Debug/Release), lie `VhfKm.lib`, avec `SignMode=Off` pour empecher la creation automatique de certificats de test. Aucun INF ni deployeur.
- `build-wdk.ps1` : compile uniquement ce projet avec MSBuild ; aucune installation ou signature.

- `src-tauri/src/virtual_pad/vhf_protocol.rs` : encodeur Rust pur, sans
  ouverture du pilote ni interaction avec Windows. Tests unitaires inclus.
- `src-tauri/src/virtual_pad/selected_backend.rs` **reste sur
  `NullBackend`**. Il n'existe pas encore de service de liaison au pilote.

## Contrat du rapport (identique cote Rust et C)

Octets : `[id=1, buttons_lo, buttons_hi, hat, lx, ly, rx, ry, lt, rt]`.

- **Boutons** : bits 0-10 = A, B, X, Y, LB, RB, Back, Start, Guide, LS, RS.
  Les autres bits sont reserves et rejetes par le pilote.
- **Hat** : 0=haut,1=haut-droite,2=droite,3=bas-droite,4=bas,
  5=bas-gauche,6=gauche,7=haut-gauche,8=neutre.
- **Axes** : LX/LY/RX/RY signes, -127 a +127. Gachettes : 0 a 255.
- **IOCTL** : METHOD_BUFFERED, FILE_WRITE_DATA, longueur exactement 10.
  La fermeture du handle envoie un rapport neutre ; cette securite partielle
  ne remplace pas un watchdog et un controle exclusif de la pile PnP.
  Le pilote rejette l'ID invalide, les bits reserves, hat > 8 et axes -128.
- Aucun identifiant VID/PID de Sony, Microsoft ou d'un tiers n'est forge.
- Aucune commande de sortie HID et aucune commande de vibration n'existe
  dans ce prototype.

## Securite et etapes de construction

**Avant tout test noyau dans Hyper-V : creer un point de controle de la VM (`PadBridge - Windows propre`). La premiere etape est uniquement la compilation du code sans charger le pilote.**

**Ne pas effectuer de commande `devcon install`, `pnputil /add-driver`,
`bcdedit /set testsigning on` ou desactivation de Secure Boot** sur
le PC personnel. Aucun package .INF n'est encore fourni ; ce dossier
ne constitue PAS un pilote installable.

Etapes a realiser **seulement sur un environnement Windows 11 isole,
avec snapshot et outil de debogage noyau** :

1. Dans l'environnement **de build**, installer Visual Studio avec
   **Desktop development with C++**, le Windows SDK et le Windows Driver Kit,
   tous dans une combinaison compatible.
   - Pour **Visual Studio 2022** : WDK + SDK **26100.6584** (Microsoft).
   - Pour **Visual Studio 2026** : WDK + SDK **28000.2526** (Microsoft).
   - Installer egalement le composant d'integration **Windows Driver Kit**
     dans Visual Studio Installer → Individual components.
   - Documentation officielle : https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk
2. Le projet a passe sa premiere compilation WDK x64 sur GitHub Actions ([run 39510109665](https://github.com/saad1937-berra/padbridge/actions/runs/39510109665)). Pour compiler localement si les outils sont disponibles, ouvrir `PadBridgeVhf.vcxproj` dans Visual Studio et choisir
   **Debug | x64 → Build**. Alternativement, depuis PowerShell dans le
   dossier `driver/poc-vhf` :
   ```powershell
   .\\build-wdk.ps1 -Configuration Debug
   ```
   La commande **compile uniquement** et ne cree ni package INF signe, ni
   machine virtuelle, ni pilote actif. Le projet C n'a **pas encore** ete
   compile avec WDK sur GitHub Actions ; les tests noyau restent a faire. Si une erreur survient, transmettre
   les **premieres erreurs** MSBuild (pas besoin de modifier les parametres
   de securite de Windows).
3. Preparer ensuite, apres revue, un package INF pour un appareil de test
   **root-enumerated**, avec `vhf` en `LowerFilters`, et un mecanisme
   de signature adapte a la VM. Etudier la compatibilite de ce montage
   avec le modele PnP et de puissance.
4. La fermeture du handle client declenche deja une tentative de remise
   a l'etat neutre ; cette logique reste **non verifiee sous WDK**.
   Ajouter imperativement un **watchdog sur absence de nouvelles trames**,
   gerer les transitions D0, les collisions avec la suppression PnP et la
   concurrence, avant tout essai d'injection d'un rapport non neutre.
5. Ecrire un **broker local restreint** qui traduira les `OutputState`
   du moteur en rapports VHF. L'interface noyau est limitee a SYSTEM /
   administrateurs : **ne jamais elever directement la fenetre Tauri**,
   ni ouvrir l'IOCTL d'injection a tout utilisateur.
6. Test de securite en VM : rapport neutre, boutons, diagonales,
   validation des champs malformes, redemarrage, debranchement, crash
   du broker, veille/reprise, suppression du pilote sans gamepad fantome.
7. **Etape XInput distincte** : etudier et justifier une interface reconnue
   par `XInputGetState`, verifier l'utilisation reelle de XInput et la
   distribution/signature du pilote. Ne pas appeler un gamepad HID
   « Xbox » avant cette validation.

## Portee du build WDK

Le build a genere `PadBridgeVhf.sys` sur un runner GitHub Windows 2022 (19 456 octets, SHA-256 : `051E73108816D69A1B1F6D0DB9AD765A68B5F0CCA3E9C922D18697CB137C22C6`). En environnement WDK local, il generera ce fichier **uniquement si le code passe la compilation**. Le projet definit
explicitement `SignMode=Off` ; le binaire qui en resulte, s'il existe,
sera **non signe et impropre au chargement tel quel**. Aucune installation
automatique n'existe. L'absence d'INF est volontaire tant que le watchdog,
la gestion PnP et le broker privilegie n'ont pas ete audites. Compiler
n'implique **ni execution ni chargement en memoire noyau**.

Le workflow separe [`.github/workflows/driver-wdk-ci.yml`](../../.github/workflows/driver-wdk-ci.yml) utilise le WDK 26100 preinstalle sur `windows-2022` pour compiler le code C, verifier l'existence du `.sys` non signe et publier un artefact temporaire (24 h). Il n'installe ni n'execute le pilote. Une compilation reussie **ne valide pas le comportement en noyau**.

## Validation possible aujourd'hui, sans pilote

Depuis le projet :

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features virtual_pad::vhf_protocol
```

Les tests ne requierent ni WDK ni manette physique. Ils verifient le
rapport neutre, l'ordre des boutons, les huit directions, les directions
opposees, le calibrage borne des axes et l'absence d'effet de bord.

## References Microsoft

- [Write a HID source driver using VHF](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/virtual-hid-framework--vhf-)
- [VhfCreate](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/vhf/nf-vhf-vhfcreate)
- [VhfReadReportSubmit](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/vhf/nf-vhf-vhfreadreportsubmit)
- [WdfDeviceInitAssignSDDLString](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdfdevice/nf-wdfdevice-wdfdeviceinitassignsddlstring)
- [DirectInput and XUSB devices](https://learn.microsoft.com/en-us/windows/win32/xinput/directinput-and-xusb-devices)

Ce POC est **en avance sur la validation WDK**, et non un pilote pret
pour la production. L'installation des pilotes et la signature Windows
n'entrent pas dans le MVP actuel.
