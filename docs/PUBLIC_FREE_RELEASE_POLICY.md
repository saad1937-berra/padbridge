# PadBridge — politique de distribution gratuite au public

**Décision produit du 22 octobre 2026 :** l'application finale sera **gratuite et téléchargeable par tous**. Ceci concerne le prix et l'accès utilisateur ; le choix de publication du code source (open source ou non) reste à décider et à documenter avant la première distribution.

## Engagements produit

- **Prix de téléchargement et d'utilisation : 0**, sans formule payante imposée aux joueurs.
- **Pas de compte obligatoire ni clé d'activation payante** pour utiliser PadBridge. Des dons pourraient être facultatifs, sans débloquer de fonction essentielle.
- Pas de pilote sous licence payante **obligatoire** pour accéder au mode Xbox de l'application distribuée.
- Licence de PadBridge et notices des dépendances à publier clairement avant distribution ; « gratuit » ne donne pas automatiquement le droit de redistribuer un autre logiciel.
- Possibilité d'installation, réparation et suppression propre sous Windows, sans modification silencieuse du Bluetooth ou du support natif de la manette.

## Fournisseurs de contrôleurs Xbox : critères incontournables

Pour accepter un backend, il faut simultanément :

1. Prouver la compatibilité **XInput/XUSB** réelle sur Windows et dans des jeux représentatifs : un simple périphérique HID générique n'est pas suffisant.
2. Avoir des droits documentés **pour l'utilisateur final et pour la redistribution publique du package**, y compris le pilote, le broker et les binaires associés. Vérifier les conditions d'utilisation de chaque version distribuée.
3. Éviter un paiement, compte ou système d'activation obligatoire chez le fournisseur pour chaque utilisateur PadBridge.
4. Vérifier la provenance, la maintenance, la signature du pilote, la compatibilité Windows, la sécurité et le support de désinstallation.
5. Fournir des tests d'installation/désinstallation et une neutralisation fiable de la manette virtuelle, sur une machine d'essai isolée.

**Conséquence immédiate : LizardByte libvirtualhid n'est pas sélectionné par défaut.** Son code client est sous MIT, mais son pilote/broker/installeur Windows est sous LB-SAL 1.0 ; la redistribution n'y est pas librement autorisée et des licences machine sont requises pour les gamepads pilotés. Source : https://github.com/LizardByte/libvirtualhid/blob/master/LICENSES/license-map.md et https://github.com/LizardByte/libvirtualhid/blob/master/LICENSES/LicenseRef-LizardByte-SAL-1.0.md. Une permission écrite supplémentaire ne résoudrait pas nécessairement l'exigence « sans licence utilisateur obligatoire » : il faudrait confirmer toutes les conditions. Ceci n'est pas un avis juridique.

ViGEmBus demeure un projet historiquement connu, mais officiellement abandonné. Il ne faut pas réintroduire par défaut un pilote noyau archivé sans solution maintenue, documentation de licence et validation de sécurité. Le pilote actuel PadBridge VHF fournit une sortie HID générique ; il n'est pas déclaré XInput.

**Nouveau garde-fou (octobre 2026) :** [audit HIDMaestro](HIDMAESTRO_XINPUT_EVALUATION.md) et fichier `policy/xinput-provider-review.json`. Le fait qu'un projet soit sous MIT ne dispense pas de vérifier sa procédure de signature Windows. PadBridge **n'ajoutera pas silencieusement une nouvelle autorité racine de confiance** à l'ordinateur de ses utilisateurs.

## Blocage de publication

Le fichier `policy/public-release.json` fixe les conditions de ce projet. Le contrôle Python `scripts/check_public_distribution.py` vérifie qu'une PR ne transforme pas silencieusement l'application en produit payant, n'impose pas de clé ni compte, ne réactive pas le chargement ou l'installation automatique de pilotes, et n'approuve pas une publication avant audit.

```powershell
python scripts/check_public_distribution.py --self-test
# Commande distincte de préparation à une release : échoue par conception à ce stade
python scripts/check_public_distribution.py --release
```

**Attention :** ce garde-fou vérifie uniquement la politique déclarée, pas chaque octet d'un futur installateur ni la conformité juridique des dépendances. Avant tout GitHub Release public, il faudra : audit SBOM/tiers, licences/notices, vérification et signature des installateurs, validation du pilote, tests en environnement isolé et décision explicite de diffusion.

## Où en est le développement ?

- Le moteur de profil, la conversion DS4 → état `XINPUT_GAMEPAD`, l'IPC et le cycle de vie des sessions sont testés en simulation.
- Le mode Xbox reste volontairement **indisponible** dans l'application tant qu'aucun fournisseur réellement redistribuable et compatible n'a été validé.
- Le service expérimental est dry-run et le pilote maison KMDF/VHF ne doit pas être chargé sur le PC principal.
- Les Pull Requests sont empilées en brouillon ; la distribution publique n'est **pas encore autorisée**.

## Futur jalon

Retenir une architecture Xbox/XUSB redistribuable gratuitement, puis produire un pilote/package installable et sécurisé dans les conditions applicables. Un fonctionnement réel dans les jeux doit être mesuré avant de déclarer PadBridge prêt pour le public.
