# PadBridge — décision de fournisseur XUSB et barrière de publication

Statut : **aucun fournisseur validé ; intégration et distribution bloquées**.
Ce dossier documente la décision, pas une autorisation d'installer un pilote.

## Besoin produit
PadBridge doit rester gratuit pour les utilisateurs et distribuable sur Windows 10/11 sans demander d'activer le mode test, de désactiver la vérification des signatures, d'importer une autorité de certification non fiable, ni de modifier un jeu. Le support Xbox virtuel n'est annoncé **que lorsqu'un vrai slot XInput a été mesuré**.

## Candidats — vérification initiale (22 octobre 2026)

| Candidat | Éléments vérifiables | Verdict provisoire |
| --- | --- | --- |
| ViGEmBus / ViGEmClient (Nefarius) | Projet officiel archivé et retiré en 2023 ; code source BSD-3-Clause ; présence historique de binaires signés. Documentation du mainteneur : https://docs.nefarius.at/projects/ViGEm/End-of-Life/ et https://github.com/nefarius/ViGEmBus | **Non retenu pour une nouvelle intégration publique** sans solution démontrée de maintenance, mises à jour et redistribution des binaires. Licence du code source ≠ autorisation automatique pour toute chaîne binaire. |
| HIDMaestro | Rejet temporaire lors de l'audit de la PR #16. Le modèle de certificat racine de confiance local doit être traité comme une surface sensible. Source : https://github.com/hifihedgehog/HIDMaestro | **Bloqué** tant que chaîne de confiance et modèle de sécurité ne sont pas acceptables. |
| libvirtualhid | La documentation annonce des MSI signés et un compagnon XUSB, mais précise qu'une **licence machine** est requise pour créer un périphérique réel. Source : https://github.com/LizardByte/libvirtualhid/blob/master/docs/windows-driver.md | **Bloqué** : évaluer coût/licence, compatibilité avec une distribution gratuite, provenance et droits d'intégration. |
| Pilote propre PadBridge | Exige WDK, signature de production reconnue, maintenance, analyse sécurité, installateur et désinstallateur vérifiés. | **Hors MVP** tant que coût et faisabilité ne sont pas établis. |

Aucun candidat ci-dessus n'est déclaré sûr, gratuit à redistribuer ou approuvé en production par ce document.

## Conditions nécessaires avant toute intégration (toutes obligatoires)

- [ ] Licence écrite et traçable autorisant explicitement **usage, intégration et redistribution** des binaires/pilotes par PadBridge, y compris la distribution gratuite. Conserver preuve et obligations de notices.
- [ ] Origine officielle, version verrouillée, SHA-256 des binaires, signature Authenticode et chaîne de confiance Windows vérifiées sur machine isolée ; aucune racine auto-signée installée silencieusement.
- [ ] Compatibilité testée Windows 10/11 x64 avec Secure Boot et HVCI/Memory Integrity lorsqu'activés ; aucun contournement ni désactivation de protection.
- [ ] Contrat d'API versionné qui permet create / submit / release et retourne des erreurs explicites, sans chargement arbitraire de DLL ni API cachée.
- [ ] Service local authentifié par identité vérifiée (SCM + identité du binaire/signature + token de service), et non par un PID client annoncé ou le PID seul.
- [ ] Perte du périphérique, du pipe, du processus, du Bluetooth ou du heartbeat : retour neutre, arrêt des sorties et nettoyage garanti ; reprise explicite.
- [ ] Test isolé réel : une manette virtuelle visible dans joy.cpl, un slot XInput 1.4 détecté, boutons/axes/gâchettes mesurés, puis disparition complète après Close et désinstallation.
- [ ] Prévenir le double input ; tester avec Steam Input activé et désactivé sans en déduire abusivement une compatibilité du jeu.
- [ ] Installer et désinstaller proprement, sans réutiliser des pilotes présents sur la machine hôte comme preuve de distribution.
- [ ] CI et journal de validation reproductibles ; validation manuelle sur VM ou PC de test, **jamais en premier sur le PC principal**.

## Règle de livraison

**Fail closed** : tant que tous les éléments précédents ne sont pas documentés et validés, `selected_backend.rs` reste `NullBackend`, aucun pilote n'est livré, et aucune release publique ne promet l'émulation Xbox/XInput. Les builds diagnostiques demeurent distincts des livrables utilisateurs.

## Suite de travail

1. Monter une matrice de preuve pour les candidats ; obtenir les conditions officielles de redistribution, pas une supposition fondée sur une licence GitHub.
2. Sélectionner **un** candidat seulement après clôture des blocages de sécurité et licence.
3. Ajouter un adaptateur derrière l'interface existante, en feature flag explicitement désactivé par défaut.
4. Vérifier sur machine isolée, archiver résultats XInput/joy.cpl et tests de nettoyage.
5. Effectuer ensuite seulement un processus de signature, packaging, licence et publication publique.
