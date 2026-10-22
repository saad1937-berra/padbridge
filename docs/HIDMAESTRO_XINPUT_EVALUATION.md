# Étude fournisseur XInput — HIDMaestro 2.0.x (22 octobre 2026)

**Statut pour PadBridge : CANDIDAT NON APPROUVÉ. Aucune installation, aucun appel SDK et aucun fichier binaire intégré.**

## Méthode

Audit documentaire du dépôt éditeur [HIDMaestro](https://github.com/hifihedgehog/HIDMaestro), à l'instantané Git `63cc6481f9e82fc8600bda8aaa65cb3271113a19` (message « Set the version to 2.0.3 »), et des exigences Microsoft de signature des pilotes. **Aucun essai sur un Windows réel ni vérification indépendante XInput n'a été exécuté.** Les résultats de validation publiés par le fournisseur restent ses propres affirmations.

## Constats

| Critère | Lecture / preuve documentaire | Décision PadBridge |
|---|---|---|
| Licence principale | Fichier [LICENSE](https://github.com/hifihedgehog/HIDMaestro/blob/63cc6481f9e82fc8600bda8aaa65cb3271113a19/LICENSE) sous MIT, avec obligation de conserver le copyright et la notice ; le package SDK peut embarquer des composants tiers (USB/IP), à auditer séparément | **Potentiellement compatible avec la gratuité**, mais inventaire de dépendances et licences manquant |
| Xbox / XInput | Le README décrit un compagnon XUSB et une création `xbox-360-wired` visible dans XInput. L'API .NET donne `HMContext`, `InstallDriver`, `CreateController`, `SubmitState` | Piste fonctionnelle crédible **mais uniquement revendiquée par l'éditeur** ; aucune preuve de compatibilité PadBridge/EA FC 26 |
| Installation | UMDF2 **est bien un pilote Windows en mode utilisateur** (ne pas confondre « pas de pilote noyau » avec « pas de pilote »). `InstallDriver` installe les packages PnP et compagnon XUSB avec privilèges élevés | Refus de l'installation silencieuse ; évaluer signature, autorisations et retrait |
| Signature / confiance | La procédure éditeur crée `CN=HIDMaestroTestCert`, ajoute un certificat autosigné aux magasins **LocalMachine Root** et **TrustedPublisher**, puis signe le package. Voir [documentation installateur](https://hidmaestro.org/docs/reference/driver-install-and-signing/) et [procédure de retrait](https://github.com/hifihedgehog/HIDMaestro/blob/63cc6481f9e82fc8600bda8aaa65cb3271113a19/docs/UNINSTALL.md) | **BLOQUANT** pour une installation automatique grand public : modification de l'ancrage de confiance système ; exiger une chaîne de signature et une revue sécurité adaptées à la distribution |
| Uninstall / coexistence | L'outil de suppression retire des pilotes et certificats **partagés entre applications HIDMaestro** ; refuse si un autre consommateur tourne, avec mécanismes de retrait spécifiques pour usbip-win2 | Exiger test de coexistence et démontrer que désinstaller PadBridge ne casse aucun autre produit |
| Tests et CI | Le README annonce des tests multi-API et Windows physiques, mais PadBridge ne les a pas reproduits ; les workflows GitHub principaux ne fournissent pas à eux seuls de preuve de signature publiquement acceptable | Test matériel et preuve d'identité PnP requises avant le moindre statut « disponible » |

Références Microsoft : [Signer un pilote](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/signing-a-driver), [offres de signature Windows](https://learn.microsoft.com/windows-hardware/drivers/dashboard/driver-signing-offerings), [politique des pilotes Windows](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy). Les documents décrivent des voies de signature pour les pilotes publics ; **faire confiance à une nouvelle racine locale** n'est pas équivalent à prouver l'identité d'un éditeur auprès des utilisateurs.

## Arbitrage produit

**Aujourd'hui nous n'adoptons pas HIDMaestro comme backend distribué.** La licence MIT est favorable, mais la modification des magasins de confiance et le caractère partagé du driver demandent un audit plus important avant une distribution aux joueurs. LizardByte reste non retenu pour ses licences propres ; le KMDF/VHF interne n'est pas XInput. Le compromis gratuit/sécurité est toujours **non résolu**.

Ce n'est pas un jugement définitif sur l'éditeur ni la qualité de sa solution : c'est une décision de risque propre à PadBridge en tant que futur logiciel diffusé à un large public.

## Travaux réalisables sans installer de pilote

1. Établir une nomenclature de dépendances et licences de la version exacte (et valider les règles pour redistribution publique).
2. Identifier un chemin d'installation signé pour un usage grand public **sans ajout d'autorité racine par l'application**, à discuter avec le mainteneur ou à concevoir et auditer indépendamment.
3. Définir un transport d'adaptation maintenu et des tests d'erreurs contrôlés, sans activer `selected_backend.rs`.
4. Dans une machine Windows isolée à une phase ultérieure : tests de signature, installation, désinstallation, compatibilité avec des applications existantes, provenance PnP du XUSB et réception réelle de rapports par XInput.

### Garde-fou implémenté

`policy/xinput-provider-review.json` enregistre les candidats, leurs risques et **l'absence de fournisseur sélectionné**. `scripts/check_xinput_provider.py --self-test` refuse toute approbation prématurée ou régression flagrante (ajout d'une racine de confiance caché, suppression du coût de licence, activation de l'installateur, déclaration fictive de XInput). Le script ne lit pas les certificats de la machine et ne constitue **ni un audit exhaustif du code Rust, ni une analyse de binaires tiers**.

**Conclusion :** conserver le backend `NullBackend` dans PadBridge. Pas de téléchargements ou de pilotes embarqués dans l'installateur actuel.
