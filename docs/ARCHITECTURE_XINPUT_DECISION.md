# PadBridge — décision d'architecture XInput / XUSB (ADR-001)

Statut : **XInput non démontré ; blocage produit avant toute promesse de compatibilité Xbox**.
Date : 2026-10-22. Cette décision ne modifie pas le backend actif.

## Constat vérifié

- Microsoft documente VHF comme une manière de fournir un périphérique **HID virtuel** et ses rapports. Cela ne suffit pas à lui seul pour obtenir une interface XUSB compatible avec les fonctions XInput.
- Le document Microsoft « DirectInput and XUSB Devices » distingue un **pilote XUSB** exportant l'interface noyau consommée par la DLL XInput et son exposition HID supplémentaire.
- `XInputGetState` interroge au maximum **quatre slots**, numérotés 0 à 3. Un slot connecté n'indique pas l'origine du périphérique.
- Notre pilote KMDF/VHF POC construit actuellement un **gamepad HID générique**. Notre Rust encodeur, le broker et les tests de format sont valides en simulation. Rien ne prouve que cette architecture seule crée un contrôleur détecté par XInput.

Références Microsoft :

- https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/virtual-hid-framework--vhf-
- https://learn.microsoft.com/en-us/windows/win32/xinput/directinput-and-xusb-devices
- https://learn.microsoft.com/en-us/windows/win32/api/xinput/nf-xinput-xinputgetstate

## Décision immédiate

**Ne pas intégrer le backend VHF au mode « Xbox/XInput » dans l'interface PadBridge** tant que des preuves d'une implémentation XInput réelle ne sont pas disponibles dans une VM isolée. `selected_backend.rs` doit rester sur `NullBackend`.

Le chemin de travail est désormais :

1. Obtenir une **baseline XInput** dans la VM propre, sans pilote PadBridge chargé.
2. Préparer/valider séparément une solution de virtualisation du contrôleur réellement reconnue par XInput, sans modifier les politiques de sécurité du PC principal.
3. Vérifier l'installation, l'identité PnP, la signature, l'usage prévu et le modèle de licences de la solution sélectionnée.
4. Capturer des snapshots XInput **avant**, **après connexion à vide** et **après activité de test contrôlée**, dans la VM, puis comparer. Un nouveau slot et des boutons changeants ne prouvent pas à eux seuls l'attribution PadBridge.
5. Établir l'origine PnP indépendamment des slots XInput, puis valider l'arrêt, la déconnexion, les rapports neutres, la reprise après veille et l'installation/désinstallation avant toute sortie grand public.

## Options d'architecture à étudier

| Option | Adéquation | Risques et conditions |
|---|---|---|
| Pilote VHF HID générique actuel | Bon socle HID/DirectInput, diagnostic et tests de format | **Ne présume pas XInput** ; ne pas le présenter comme une manette Xbox |
| Backend tiers **maintenu et signé**, qui expose vraiment Xbox/XUSB | Candidat pour un vrai mode Xbox | Audit de la provenance, maintien, licence, chaîne de distribution, autorisations, sécurité et support des jeux ; choix non effectué |
| Pilote XUSB maison | Peut répondre au besoin seulement après conception et validation dédiées | Effort élevé, compatibilité et interface Windows à étudier, signature/installation et risques noyau ; ne pas improviser |
| DLL XInput modifiée ou injectée dans chaque jeu | Non retenue comme architecture par défaut | Comportements variables, interférences avec intégrité/anti-triche, contournements fragiles |

Des solutions historiques telles que ViGEmBus existent mais leur maintenance, leur licence et leur support Windows doivent être revérifiés au moment du choix. **Ne pas automatiquement télécharger ni installer de pilote tiers sur l'hôte.**

## Preuve instrumentée et limites

- `padbridge-xinput-probe --snapshot` lit quatre slots XInput **une seule fois**, sans vibration ni injection.
- `scripts/collect_vm_xinput_baseline.ps1` refuse de fonctionner hors invité Microsoft Hyper-V identifié et se limite à enregistrer ces snapshots (le checkpoint doit être créé depuis l'hôte par l'utilisateur).
- `scripts/compare_xinput_snapshots.py` valide strictement les trois documents JSON, puis émet un verdict **candidat non attribué**, jamais « PadBridge fonctionne sur XInput ». Il refuse les données inconnues et ne conserve ni nom d'utilisateur, ni ID PnP, ni numéro de série.
- Un changement du **seul** numéro de paquet ne suffit pas : les touches, axes ou gâchettes doivent effectivement changer.

## Critères de sortie de ce blocage

1. Backend choisi avec autorisation de redistribution, maintenance et signature documentées.
2. Contrôleur visible et attribué par une vérification PnP **indépendante** dans la VM.
3. `XInputGetState` détecte le contrôleur et la lecture reflète les entrées de test, sans confondre une manette préexistante.
4. Absence de bouton bloqué et comportement sûr lors de l'arrêt, de l'expiration 600/800 ms et de la reconnexion.
5. Tests dans les jeux cibles sur un environnement séparé, puis approbation des conditions d'installation sur Windows 11.

**Statut actuel : critères 1 à 5 non atteints.** Ce document prépare la décision, mais ne constitue pas une validation noyau, XInput ou EA FC 26.
