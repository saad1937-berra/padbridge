# PadBridge — lot XInput v1 : préparation vérifiable

Statut : **recherche / diagnostic uniquement**. Aucun pilote sélectionné, installé ou redistribué par ce lot.

## État initial (22 octobre 2026)

- `src-tauri/src/virtual_pad/selected_backend.rs` retourne `NullBackend`.
- Les contrats `xinput_state_contract.rs`, `xbox_provider_adapter.rs` et `xbox_wire.rs` sont des fondations de simulation, **pas** une preuve de contrôleur Xbox visible par un jeu.
- `driver/poc-vhf` crée un prototype HID générique ; VHF ne garantit pas une interface XUSB/XInput.
- `policy/public-release.json` interdit la publication et `policy/xinput-provider-review.json` ne valide aucun candidat.

## Objectifs du premier lot

1. Émettre un **rapport statique en lecture seule**, à partir des politiques du dépôt et du backend sélectionné.
2. Signaler explicitement tout décalage entre les deux politiques ou l'activation d'un backend sans autorisation.
3. Disposer d'un rapport JSON exploitable par CI et par la future interface de diagnostic.
4. Garder une sortie **fail closed** : en l'absence de preuves, `ready_for_public_xinput` reste `false`.

Commande depuis la racine du dépôt :

```powershell
python scripts/report_xinput_readiness.py
python scripts/report_xinput_readiness.py --json
python scripts/report_xinput_readiness.py --self-test
```

Le code retour `0` signifie uniquement que les **garde-fous statiques locaux sont cohérents**. Il ne signifie **jamais** que le backend Xbox est opérationnel ou prêt à publier. Un code `1` signifie politique incohérente/lecture impossible.

## Étapes techniques suivantes — séparées

| Étape | Travail | Validation nécessaire |
| --- | --- | --- |
| A. Fournisseur | Identifier un backend XUSB maintenu et légalement redistribuable sans abonnement ni certificat racine ajouté | Licence du pilote **et** des binaires, provenance, signature, maintenance |
| B. Contrat | Créer l'adaptateur concret derrière `VirtualPadBackend` en feature flag inactif par défaut | Tests de cycle create / submit / destroy, erreurs explicites |
| C. Service | Vérifier l'authentification du service et la neutralisation à perte de heartbeat | Tests négatifs + reprise et nettoyage |
| D. Windows isolé | Installer et tester sur VM/PC de test (pas PC principal) | Identité PnP, `XInputGetState`, axes, boutons, disparition après arrêt |
| E. Distribution | Vérifier la signature, désinstallation, HVCI/Secure Boot et notices de licence | Validation Windows 10/11, puis changement des politiques |

## Invariants de sécurité

- Ne pas changer `NullBackend` avant clôture des critères du document `docs/XUSB_PROVIDER_RELEASE_GATE.md`.
- Ne pas installer de pilote, charger de `.sys`, signer en mode test ou modifier la confiance des certificats dans ce lot.
- Ne pas annoncer de compatibilité EA FC, Steam ou XInput au seul vu des tests unitaires.
- Conserver la distinction entre **diagnostic gratuit** et **émulation Xbox non encore démontrée**.

Voir aussi : `docs/ARCHITECTURE_XINPUT_DECISION.md` et `policy/xinput-provider-review.json`.
