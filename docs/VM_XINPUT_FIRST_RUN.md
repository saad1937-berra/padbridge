# PadBridge — premier essai XInput dans Hyper-V (lecture seule)

Ce kit autonome ne contient que `padbridge-xinput-probe.exe`, un script de collecte PowerShell et le comparateur Python. Il **ne contient pas** de pilote `.sys`, ne charge ni service ni pilote, n'injecte aucune entrée et ne modifie ni Bluetooth ni les manettes.

## Télécharger le kit depuis GitHub

1. Ouvrir le workflow `PadBridge Windows CI` de la PR #10 dans GitHub Actions.
2. Sur la **dernière exécution réussie** de cette branche, section `Artifacts`, télécharger `PadBridge-XInput-ReadOnly-Windows-x64`.
3. Dans la VM **Windows 11 Hyper-V déjà installée**, extraire les fichiers dans un dossier dédié. Ne pas exécuter le kit sur le Windows principal.

Ne pas contourner aveuglément une alerte de sécurité Windows. Vérifier l'origine du workflow, l'extraction correcte du fichier et sa somme SHA-256 si nécessaire. Il s'agit d'un prototype diagnostic compilé en CI, **non signé**.

## Prérequis

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

Documentation complémentaire : `VALIDATION_XINPUT_VM.md` et `ARCHITECTURE_XINPUT_DECISION.md`.
