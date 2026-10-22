# PadBridge — premier essai XInput dans Hyper-V (lecture seule)

Ce kit autonome ne contient que `padbridge-xinput-probe.exe`, un script de collecte PowerShell et le comparateur Python. Il **ne contient pas** de pilote `.sys`, ne charge ni service ni pilote, n'injecte aucune entrée et ne modifie ni Bluetooth ni les manettes.

## Télécharger le kit depuis GitHub

1. Sur GitHub, ouvrir [Actions — PadBridge Windows CI](https://github.com/saad1937-berra/padbridge/actions/workflows/windows-ci.yml) et choisir une **exécution réussie sur `main`**, de préférence le commit fusionné le plus récent. Ne pas se limiter à une ancienne PR : les artefacts ont une durée de conservation limitée.
2. Dans `Artifacts`, télécharger `PadBridge-XInput-ReadOnly-Windows-x64` (si disponible ; sinon attendre une nouvelle exécution de CI). Contrôler que le workflow et le dépôt sont bien ceux de PadBridge.
3. Extraire le kit **dans la VM Windows 11 Hyper-V**, dans un dossier dédié. Ne pas exécuter le kit sur le Windows principal.

Ne pas contourner aveuglément une alerte de sécurité Windows. Vérifier l'origine du workflow, l'extraction correcte du fichier et sa somme SHA-256 si nécessaire. Il s'agit d'un prototype diagnostic compilé en CI, **non signé**.

## Prérequis

- Installer Windows 11 **dans une machine virtuelle de génération 2**, avec Secure Boot compatible Windows et ressources adaptées, en respectant les exigences d'installation de Windows 11. Créer un point de contrôle depuis le gestionnaire Hyper-V avant les tests de contrôleurs virtuels.
- Garder les tests de pilotes et services expérimentaux en dehors du PC principal ; une VM Hyper-V ne garantit pas un passage direct du Bluetooth/USB hôte vers la VM.
- Pour cette première phase, **aucun transfert de manette physique n'est nécessaire** : la capture de base XInput se fait sans contrôleur.

- VM démarrée, un **point de contrôle créé dans Hyper-V depuis l'hôte** avant tout essai ultérieur.
- Pas de manette virtuelle ni de périphérique Xbox de test encore installé dans la VM.
- PowerShell dans la VM ; aucune élévation demandée pour les captures.

## Première capture (à faire maintenant)

Dans PowerShell, dans le dossier extrait :

```powershell
.\collect_vm_xinput_baseline.ps1 -Stage before
```

Le script vérifie qu'il s'exécute dans un invité Microsoft Hyper-V Windows 11, lit les quatre slots XInput, crée `vm-evidence\before.json` à côté du dossier et **refuse d'écraser** un fichier existant. Si l'hôte n'est pas reconnu comme VM, il s'arrête sans interroger XInput.

Alternative manuelle si PowerShell interdit l'exécution des scripts :

```powershell
.\padbridge-xinput-probe.exe --snapshot
```

Ne pas désactiver globalement l'Execution Policy pour ce test. Le programme manuel ne sauvegarde pas le résultat : on peut consulter la sortie JSON.

## Étapes différées : uniquement après validation d'une véritable émulation XInput dans la VM

```powershell
.\collect_vm_xinput_baseline.ps1 -Stage after-idle
# Simuler une action sur le contrôleur virtuel validé, puis :
.\collect_vm_xinput_baseline.ps1 -Stage after-activity
python .\compare_xinput_snapshots.py --before .\vm-evidence\before.json --after-idle .\vm-evidence\after-idle.json --after-activity .\vm-evidence\after-activity.json
```

Un nouveau slot seul ne prouve pas qu'il appartient à PadBridge. Même avec des boutons changeants, l'attribution doit être confirmée séparément par les informations PnP. Ne pas installer le `.sys` expérimental dans l'hôte ni dans la VM avant le contrôle d'installation/signature et un audit noyau.

## Quatrième capture : fermeture (plus tard, après validation d'un fournisseur)

Une fois le contrôleur de test correctement fermé **dans la VM**, sans installer quoi que ce soit sur l'hôte :

```powershell
.\collect_vm_xinput_baseline.ps1 -Stage after-close
python .\check_xinput_cleanup.py --self-test
python .\check_xinput_cleanup.py --before .\vm-evidence\before.json --after-idle .\vm-evidence\after-idle.json --after-activity .\vm-evidence\after-activity.json --after-close .\vm-evidence\after-close.json
```

L'outil vérifie uniquement les quatre snapshots JSON et distingue un candidat disparu d'un candidat toujours connecté. **Il ne prouve ni l'identité PnP ni la désinstallation d'un pilote, et n'autorise jamais la publication.** Le kit CI devra inclure le nouveau script avant d'utiliser cette commande depuis un ZIP autonome.

Documentation complémentaire : `VALIDATION_XINPUT_VM.md` et `ARCHITECTURE_XINPUT_DECISION.md`.
