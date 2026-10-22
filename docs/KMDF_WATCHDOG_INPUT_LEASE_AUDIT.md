# Audit POC KMDF/VHF — sécurité des boutons maintenus

**Statut : revue de code et tests simulés uniquement. Le pilote demeure non signé, non installé et non exécuté.**

## Défaut identifié côté broker

Avant la correction, `BrokerSession::heartbeat()` rappelait `send_validated()` avec le dernier rapport non neutre. Cette fonction actualisait le temps de dernière émission, mais aucune date indépendante de dernière **entrée physique fraîche** n'était conservée. Un broker encore vivant mais privé de nouvelles entrées pouvait donc renvoyer indéfiniment le dernier bouton appuyé et empêcher l'expiration du watchdog noyau de 800 ms.

## Correction Rust

- `BrokerSession` conserve désormais `last_input` (date d'une trame fraîche, distincte de `last_sent`).
- Tout rapport neuf validé et effectivement envoyé renouvelle `last_input`. Un keepalive ne le renouvelle **jamais**.
- `INPUT_LEASE = 600 ms` (strictement inférieur au watchdog noyau à 800 ms). Si une session active atteint ce délai sans nouvelle entrée, `heartbeat()` tente un rapport neutre, marque la session fermée et renvoie une erreur `TimedOut`.
- Sur échec d'envoi du keepalive, le broker tente aussi une fermeture avec trame neutre et conserve l'erreur initiale.
- Après fermeture, un autre rapport ou heartbeat doit échouer ; la destruction n'envoie pas de nouvelle commande active.

**Limite essentielle :** `heartbeat()` doit être appelé régulièrement par un futur ordonnanceur de service. Cet ordonnanceur n'existe toujours pas. Si le processus plante ou si sa boucle se bloque, seul le watchdog du noyau peut agir. En cas d'échec d'écriture, l'envoi neutre n'est qu'une *tentative* ; il ne garantit pas à lui seul la libération des boutons.

## Défaut identifié côté pilote KMDF

Dans `PbEvtFileCleanup`, une remise à zéro de `HasActiveInput` était effectuée **même si** `PbSubmitNeutralLocked()` échouait. Ceci pouvait empêcher le watchdog encore actif de réessayer la neutralisation, laissant une commande active dans l'enfant HID.

Le code ne remet plus ce drapeau à zéro de manière inconditionnelle lors de `FileCleanup`. `PbSubmitNeutralLocked()` efface déjà `HasActiveInput` **uniquement lorsque `VhfReadReportSubmit` retourne une réussite**. En cas d'échec et lorsque le périphérique reste alimenté, le watchdog conserve sa condition de réessai.

`D0Exit` conserve sa propre logique de mise hors tension et d'arrêt du timer. Une neutralisation ne doit pas être promise lorsque l'alimentation est absente ; `D0Entry` tente une nouvelle neutralisation lors de la reprise.

## Régressions automatisées

| Scénario | Contrôle |
|---|---|
| Rapport actif puis keepalive | rapport actif répété, date de dernière entrée inchangée |
| Flux physique arrêté au-delà de 600 ms | session fermée, tentative de rapport neutre, erreur `TimedOut` |
| Nouvelle trame fraîche avant expiration | bail de 600 ms renouvelé |
| Commande neutre sans bouton maintenu | pas d'écriture supplémentaire au heartbeat |
| Erreur de transport pendant l'envoi | tentative de neutralisation et session fermée |
| Contrat watchdog | 200 ms de cadence nominale, 600 ms de bail entrée, 800 ms de secours noyau |
| C pilote / WDK | recompilation WDK sur GitHub CI, sans chargement |

Les tests simulent les temps avec `Instant` modifié en mémoire : aucune attente ni entrée matérielle n'est nécessaire.

## Risques encore ouverts (bloquants avant tout essai VM)

1. Garanties KMDF sur `VhfReadReportSubmit`, `VhfDelete` et la synchronisation avec les callbacks PnP lors d'un retrait surprise : non validées en exécution noyau.
2. Reprise après veille, `D0Exit/D0Entry`, et suppression/récréation du périphérique : non testées dans une VM.
3. Validation du package INF, identité du périphérique, politiques de signature et installation/désinstallation : non fournies.
4. Ordonnanceur de heartbeat et récupération des pertes de trames physique dans un service réellement installé : non implémentés.
5. Exposition HID générique **distincte** d'une compatibilité XInput/XUSB : cette dernière n'est pas démontrée.

Ne pas installer le `.sys` sur le PC principal ni modifier le Bluetooth ou la DualShock 4. Les prochaines étapes passent par une VM Hyper-V isolée avec point de contrôle et un audit de sécurité du pilote avant toute tentative de chargement.
