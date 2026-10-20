# POC B — pilote virtuel PadBridge (KMDF + VHF)

**Etat : code source experimental, NON compile avec WDK et NON installe. Ne PAS l'installer sur le Windows 11 principal.**

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

**Ne pas effectuer de commande `devcon install`, `pnputil /add-driver`,
`bcdedit /set testsigning on` ou desactivation de Secure Boot** sur
le PC personnel. Aucun package .INF n'est encore fourni ; ce dossier
ne constitue PAS un pilote installable.

Etapes a realiser **seulement sur un environnement Windows 11 isole,
avec snapshot et outil de debogage noyau** :

1. Installer Visual Studio + Windows Driver Kit dans l'environnement **de build**.
   Creer un projet **KMDF Driver, Empty**, ajouter `PadBridgeVhf.c/.h`,
   inclure `Vhf.h` et lier `VhfKm.lib`. Ne pas installer le pilote.
2. Compiler pour x64, activer warnings/analyses statiques WDK et corriger
   toutes les erreurs. Le code n'a **pas encore** ete compile avec le WDK.
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
