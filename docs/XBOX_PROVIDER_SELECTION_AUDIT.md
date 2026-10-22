# Fournisseur Xbox virtuel : audit 2026 et adaptateur indépendant

## Décision de sécurité et de licence — 22 octobre 2026

**Aucun fournisseur externe n'est sélectionné, installé, démarré ni redistribué par PadBridge.** Cette analyse est technique, pas un avis juridique. Les éléments ci-dessous sont à revalider avant toute distribution.

### LizardByte libvirtualhid

- Référentiel éditeur : https://github.com/LizardByte/libvirtualhid.
- La bibliothèque C++ publique (API client, en-têtes, exemples) relève principalement de **MIT** ; le pilote UMDF2, le broker, les modules de licences/entitlements et les archives MSI relèvent de la **LizardByte Source-Available License 1.0 (LB-SAL)**. La carte des licences fait foi : https://github.com/LizardByte/libvirtualhid/blob/master/LICENSES/license-map.md.
- La licence **LB-SAL n'est pas open source** : elle interdit notamment la redistribution du logiciel et des dérivés, sauf autorisation distincte ; ses restrictions comportent des clauses spécifiques aux usages commerciaux et concurrents. Source : https://github.com/LizardByte/libvirtualhid/blob/master/LICENSES/LicenseRef-LizardByte-SAL-1.0.md.
- Le produit Windows requiert une **licence machine** pour créer des contrôleurs pilotés. **Acheter une licence d'utilisation ne garantit pas l'autorisation de redistribuer son MSI avec PadBridge**. Autorisation écrite/contrat distinct nécessaires avant tout bundling, et la compatibilité PadBridge avec le produit doit être confirmée auprès de l'éditeur.
- La documentation `docs/windows-driver.md` du dépôt actuel indique un **compagnon XUSB du profil Xbox 360**, tandis que la page marketing `Virtual HID Driver` décrit encore un chemin UMDF/VHF sans Xbox 360. Cette divergence peut traduire un décalage de version ou de contenu ; il faut valider la version exacte, la release signée et le comportement réel, pas extrapoler.
- GitHub publiait le **20 octobre 2026** une release préliminaire `v2026.1020.1752.19` comportant un installeur Windows AMD64. Son statut *prerelease* ne vaut pas validation de compatibilité Windows, XInput ni EA FC 26.
- Le fournisseur avertit que son support XUSB vise un protocole **non documenté publiquement par Microsoft** et demande des tests de régression par version Windows. À prendre en compte dans la maintenance.

### Autres pistes

- **ViGEmBus** : projet historiquement efficace pour les manettes Xbox 360, mais son support officiel a pris fin. Nouvelles dépendances par défaut non recommandées sans maintenance et validation de provenance.
- **Pilote KMDF/VHF actuel de PadBridge** : descripteur HID générique, sans preuve de compatibilité XUSB/XInput. Ne pas essayer de lui transmettre directement un `XInputReport` de 12 octets.
- **Pilote XUSB maison** : coût et risque élevés ; protocole et distribution/signature noyau devraient être revus séparément. Pas un jalon réalisable simplement par une CI qui compile.

## Adaptateur Xbox préparatoire

`src-tauri/src/virtual_pad/xbox_provider_adapter.rs` définit une interface **abstraite** `XboxTransport` avec les opérations minimales `is_ready`, `open`, `submit`, `close`. Le module `XboxAdapter<T>` gère exclusivement les sessions du moteur Rust ; il ne dépend ni de LizardByte, ni de ViGEmBus, ni d'un DLL ou service Windows.

Un futur transport pourra être envisagé **uniquement après** vérification de la licence, de la provenance, de la signature, des droits d'usage, du protocole d'IPC et des timeouts.

Propriétés testées sur un transport **en mémoire** :

- Réponse `BACKEND_UNAVAILABLE` si le fournisseur n'est pas prêt ; aucune création n'est essayée.
- Au plus quatre sessions et une seule session par identifiant physique.
- Rapport neutre obligatoire dès l'ouverture, et rollback/fermeture même lorsque cette première émission échoue.
- Validation des nombres flottants avant envoi ; refus des NaN, infinis et valeurs hors plage.
- Une erreur de soumission révoque la session, tente une neutralisation, puis ferme le handle.
- `destroy()` et `Drop` tentent neutralisation + fermeture, et la fermeture est idempotente côté adaptateur.

**Limites :** si le transport futur est bloquant ou ne répond pas au neutre, cet adaptateur ne peut pas garantir la libération des boutons. Le transport réel exigera annulation, timeouts, synchronisation, contrôle de signature/provenance et validation système isolée.

## Durcissement avant un transport réel : perte fournisseur et sécurité

Le contrat `XboxAdapter<T>` traite désormais une erreur de soumission comme une **panne globale du transport partagé** : toutes les sessions sont révoquées, et chaque poignée subit une tentative de rapport neutre puis de fermeture. L'adaptateur essaie toujours les autres fermetures, même quand une neutralisation ou une fermeture précédente échoue ; les deux causes sont rapportées si elles échouent ensemble.

- `invalidate_all()` est une opération explicite et idempotente pour l'arrêt de service, la déconnexion de la source physique ou la perte du fournisseur.
- `create()` et `submit()` contrôlent la disponibilité du fournisseur et révoquent les anciennes sessions s'il n'est plus prêt.
- Les identifiants de session augmentent sans retour à zéro silencieux : en cas d'épuisement du compteur, l'ouverture est refusée **avant** toute nouvelle poignée.
- La reconnexion exige une **nouvelle session** : aucun ancien identifiant ne peut réutiliser silencieusement une poignée après une coupure.
- Les tests en mémoire injectent perte de fournisseur, échec de neutralisation et fermeture, panne sur une session parmi deux, arrêt répété et compteur épuisé.

**Limite fondamentale :** aucun appel à `is_ready()` ne peut détecter à lui seul une panne silencieuse si l'adaptateur est inactif. Le futur broker devra recevoir un signal d'arrêt/déconnexion fiable, vérifier périodiquement la santé du transport avec des opérations bornées, et imposer un watchdog indépendant capable de neutraliser les commandes même si le processus client tombe. Un `close()` qui échoue peut laisser un périphérique réel actif : nos tests ne démontrent que les **tentatives de nettoyage**. Ceci reste du code Rust sans pilote XUSB sélectionné ni entrées de jeux.

## Décision de mise en production

`selected_backend.rs` reste sur `NullBackend`. Aucune dépendance propriétaire, téléchargement MSI, installation ou activation de licence n'est autorisée dans ce lot. Le prochain choix doit être fait après clarification directe des conditions de distribution auprès du fournisseur et tests dans un environnement isolé disponible. Les tests de ce module sont un contrat d'intégration, **pas une manette Xbox fonctionnelle sous Windows**.

## Références supplémentaires

- https://github.com/LizardByte/libvirtualhid/blob/master/LICENSE.md
- https://github.com/LizardByte/libvirtualhid/blob/master/docs/windows-driver.md
- https://github.com/LizardByte/libvirtualhid/releases/tag/v2026.1020.1752.19
- https://app.lizardbyte.dev/libvirtualhid/virtual-hid-driver/
