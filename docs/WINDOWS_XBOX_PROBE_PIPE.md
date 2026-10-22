# Windows — canal local Xbox, lecture seule et identité vérifiée

**Livraison POC : service local de diagnostic « Probe » uniquement, non connecté à un pilote.** Ceci n'est ni une manette virtuelle, ni une intégration XInput dans les jeux.

## Réutilisation du mécanisme Windows existant

Le protocole binaire privé `PBXB` défini dans [XBOX_PROVIDER_WIRE_CONTRACT.md](XBOX_PROVIDER_WIRE_CONTRACT.md) dispose désormais d'un **point d'entrée Windows réel pour `Probe` seulement**, réutilisant les fonctions déjà testées du broker POC :

- Endpoint fixe `\\\\.\\pipe\\PadBridge.XboxProbe.POC.v1`, **distinct du pipe PBIP** existant et accepté comme instance unique.
- `CreateNamedPipeW` : ACL DACL protégée pour SYSTEM, Administrators et le **SID utilisateur explicitement provisionné**, sans droit de créer une autre instance de pipe pour le client ordinaire ; clients distants rejetés.
- Après réception, `ImpersonateNamedPipeClient` + lecture de `TokenUser` ; le serveur refuse tout expéditeur dont le SID effectif ne correspond pas au SID configuré (même s'il s'agit d'un compte administrateur).
- I/O asynchrone bornée : 2,5 s pour l'hôte POC, 1,5 s par écriture/lecture pour le client et 1 s pour trouver le pipe ; opérations en attente annulées et drainées avant libération des buffers.
- Contrat PBXB v1 à longueurs fixes et contrôle de séquence. `Probe` renvoie `Ok` seulement si l'identité est vérifiée et si les champs sont valides.
- Toute commande `Open`, `Submit` ou `Close` renvoie **Unavailable**, même si l'utilisateur est authentifié. Aucun appel à `XboxAdapter::create`, driver, service SCM ou accès HID/XUSB n'est possible dans cet endpoint.

Ce module est compilé uniquement derrière la feature opt-in **`broker-service-poc`**. Il n'est construit ni lancé par l'interface Tauri par défaut. L'absence d'un SID provisionné empêche l'ouverture du serveur. Aucune installation de service ni création de clé de registre n'est prévue dans ce lot.

## Tests Windows sans équipement

Dans la CI Windows GitHub, les tests créent temporairement un pipe depuis le processus de test, puis le ferment :

1. Un processus autorisé avec son vrai SID Windows obtient une réponse `Probe` valide.
2. Une requête `Open` valide reçoit `Unavailable` sans allocation de session.
3. Une version de protocole malformée reçoit `Invalid` (numéro de séquence 0).
4. Un compte non provisionné ne reçoit jamais de réponse `Ok` (le système peut refuser l'ouverture avant le message ou le serveur peut refuser le SID).
5. Les tests de l'ancien broker POC continuent de confirmer le traitement de PING et l'arrêt sur STOP.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features broker-service-poc xbox_probe
```

**Limites :** la vérification du compte Windows par SID ne garantit pas à elle seule l'authenticité du programme serveur : une future installation de service signée devra vérifier identité du processus, binaire déployé, privilèges, provenance du package, protection du pipe contre l'usurpation, et gérer un canal multi-messages et l'annulation à l'arrêt. Le POC actuel ne sert **qu'un message puis ferme le pipe**.

Une vraie émission de manette nécessitera un fournisseur XUSB/XInput approuvé, des droits de distribution, la signature et des essais isolés. `selected_backend.rs` reste sur `NullBackend`. **Aucune installation ou essai n'est à réaliser sur le PC principal.**
