# PadBridge — matrice de recherche des fournisseurs XInput/XUSB

Date de l'examen : 2026-10-22. Statut : **recherche seulement ; aucun fournisseur approuvé**.

Cette étude compare les déclarations publiques des éditeurs aux critères de PadBridge. Elle **n'autorise ni téléchargement, ni installation, ni sélection** d'un pilote sur le PC principal. Les informations indiquées comme « annoncé » ne constituent pas une validation indépendante.

## Conditions non négociables

- Application gratuite pour tous, sans licence machine ni abonnement exigé de l'utilisateur.
- Autorisation explicite de redistribuer les **binaires du pilote**, pas seulement le code de la bibliothèque.
- Paquet de production signé, provenance officielle et mises à jour sûres.
- Installation et désinstallation sans activation de `testsigning`, désactivation de Secure Boot/HVCI ou ajout d'une autorité racine autosignée.
- Preuve indépendante de l'identité PnP et de la sortie `XInputGetState` en VM/PC de test.
- Contrôle des déconnexions, entrées neutres, double input et cohabitation avec Steam Input.

## Tableau comparatif

| Option | XInput/XUSB revendiqué | Gratuité et redistribution | Signature / installation | Décision |
| --- | --- | --- | --- | --- |
| **ViGEmBus** | Oui, historiquement | Code BSD-3-Clause ; droits sur la chaîne de binaires et les notices à vérifier | Projet **archivé** ; historique d'anciens auto-updaters à risque | **Non retenu** pour une nouvelle distribution sans maintenance viable |
| **HIDMaestro** | Oui, annoncé pour profil Xbox 360 ; non vérifié ici | Code MIT annoncé ; dépendances et droits des binaires à auditer | UMDF2 avec certificat **autosigné ajouté localement à la confiance** selon la documentation et le dépôt | **Bloqué** par notre règle de confiance locale |
| **libvirtualhid** | Oui, compagnon Xbox 360/XUSB annoncé | Bibliothèque MIT ; paquet Windows et broker sous **LB-SAL 1.0** ; **licence machine** pour création de manettes | MSI de production annoncé ; Windows 11 AMD64 ciblé, Windows 10 best-effort | **Bloqué** par licence machine + redistribution non établie |
| **PadBridge VHF générique** | Non démontré ; périphérique HID n'implique pas XInput | Notre source ; licence propre PadBridge encore à choisir | Prototype de pilote **non signé**, compilation seulement | **Pas un fournisseur Xbox** ; garder comme POC |

Les observations ci-dessus sont des **indices de sélection**, pas une certification de sûreté ou une garantie commerciale.

## Sources publiques et provenance

- ViGEmBus : déclaration de fin de vie de Nefarius, https://docs.nefarius.at/projects/ViGEm/End-of-Life/ (archivage, absence de mises à jour, risque associé aux anciens updaters).
- HIDMaestro : https://github.com/hifihedgehog/HIDMaestro (revendication XInput, modèle UMDF2 et certificat autosigné local).
- libvirtualhid : https://github.com/LizardByte/libvirtualhid/blob/master/docs/windows-driver.md et https://github.com/LizardByte/libvirtualhid/blob/master/LICENSES/license-map.md (licence machine Windows, composants sous licences différentes, prise en charge annoncée).
- PadBridge : `docs/ARCHITECTURE_XINPUT_DECISION.md`, `docs/XUSB_PROVIDER_RELEASE_GATE.md`, `policy/xinput-provider-review.json`.

Les pages externes et leurs conditions peuvent changer. Avant toute autorisation, conserver date, version exacte, licence du binaire, preuve de signature, hash SHA-256 et accord de redistribution.

## Questions à faire trancher avant intégration

1. Existe-t-il un fournisseur XUSB **encore maintenu** avec signatures de production Windows et distribution gratuite autorisée ?
2. Peut-on en redistribuer légalement le pilote et toutes ses dépendances dans un installeur PadBridge public ?
3. Son modèle d'installation respecte-t-il la confiance Windows sans importer une racine non vérifiée ?
4. Fournit-il une API create / submit / destroy documentée et des sorties neutres en cas de perte de connexion ?
5. La VM propre montre-t-elle à la fois la provenance PnP et un vrai nouveau slot XInput attribuable au fournisseur ?

**Aucun candidat ci-dessus ne satisfait aujourd'hui l'ensemble de ces preuves.** Ne pas remplacer `NullBackend` ; ne pas modifier les politiques de distribution. Prochaine étape utile : examen des versions et droits de redistribution d'un candidat prometteur, puis simulation de l'adaptateur sans pilote, avant tout test isolé.
