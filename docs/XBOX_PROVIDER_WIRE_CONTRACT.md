# PadBridge — contrat privé entre moteur Xbox et futur fournisseur XUSB

**Statut : protocole d'intégration en espace utilisateur, testé sur un simulateur en mémoire. Aucun fournisseur Windows ni pilote n'est connecté.** Cette étape n'active pas XInput dans les jeux.

## Chemin logiciel désormais testable

DualShock 4 / DualSense (rapports) → Engine & profils → `OutputState` normalisé → `XInputReport` → `XboxAdapter<XboxWireClient<E>>` → **protocole privé PBXB** → `XboxExchange` simulé. Le format n'est **pas** le protocole du pilote XUSB Microsoft. Le tableau de 12 octets transmet seulement l'état normalisé de la manette au futur service que nous pourrions choisir.

## Structure binaire v1

Les octets sont transmis en ordre little-endian avec des tailles fixes. Les champs réservés doivent être nuls.

| Champ | Requête (36 octets) | Réponse (24 octets) |
|---|---|---|
| Marque | 0..3 : ASCII `PBXB` | 0..3 : ASCII `PBXB` |
| Version | 4 : `1` | 4 : `1` |
| Opération / résultat | 5 : Probe=1, Open=2, Submit=3, Close=4 | 5 : Ok=0, Invalid=1, Unauthorized=2, Unavailable=3, UnknownSession=4, Busy=5 |
| Réservé | 6..7 : zéro | 6..7 : zéro |
| Poignée | 8..15 : `u64` du fournisseur (zéro pour Probe et Open) | 8..15 : zéro pour Probe, nouvelle poignée non nulle pour Open, même poignée pour Submit et Close |
| Numéro de séquence | 16..23 : `u64` monotone non nul | 16..23 : écho exact du numéro de séquence |
| État | 24..35 : 12 octets (état non neutre seulement pour Submit) | — |

Une erreur d'encodage, un statut inconnu, un numéro de séquence réutilisé, une réponse de taille invalide, un champ réservé non nul ou une mauvaise poignée invalident la réponse. **Un accusé Ok signifie seulement que le service a répondu**, jamais que XInput a créé un slot Windows.

## Propriété de sûreté du client

- Le contrat ne recharge jamais automatiquement une requête `Open` ou `Submit` ambiguë après une déconnexion.
- Le compteur de requête refuse de dépasser `u64::MAX` plutôt que de se réinitialiser.
- La méthode `open()` envoie d'abord une requête Probe (lecture d'état), puis Open ; elle refuse une réponse avec poignée zéro.
- Les erreurs de transport ou de protocole remontent à `XboxAdapter`, qui révoque et tente de neutraliser les sessions comme défini par la PR #15.
- Les tests en mémoire vérifient deux manettes, la neutralisation, l'ordre d'ouverture/fermeture, les octets exacts et les réponses corrompues.

## Ce qui reste indispensable

1. Un **vrai processus fournisseur** disposant d'une API vérifiée (et d'une licence de redistribution gratuite).
2. Un canal local Windows concret avec ACL restrictives, identité du serveur, nonce/identité de connexion, délais d'attente bornés et annulation. **Les champs du paquet PBXB ne sont pas un mécanisme d'authentification.**
3. Un watchdog côté fournisseur qui neutralise et libère physiquement tous les contrôleurs lorsque le client disparaît. Les tests en mémoire montrent qu'un service interrompu pendant une coupure peut conserver des poignées ; une simple révocation côté client ne suffit pas.
4. Une validation PnP/XInput sur Windows isolé, ainsi qu'une vérification dans EA FC 26.
5. Une revue indépendante du choix de pilote : signature, installation, droits de redistribution, composant tiers et désinstallation.

## Exécution du test

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features xbox_wire
```

`selected_backend.rs` continue de retourner `NullBackend`. Aucun service, DLL ou pilote n'est lancé. Le contrat est **dormant** tant qu'une implémentation `XboxExchange` concrète n'est pas conçue et acceptée.
