# PB-024 — Accessibilité de l'interface PadBridge

Statut : **premiers tests clavier validés par l'utilisateur sur Windows 11 (20/10/2026) ; contrôle Narrateur/NVDA, contraste, zoom et audit restant à effectuer**. Cet ensemble de modifications ne constitue pas une certification WCAG 2.2 AA.

## Principes

- Navigation au clavier uniquement, sans souris ni manette.
- Chaque changement d'écran dirige le focus sur son titre H1 et actualise le titre de la fenêtre.
- Le premier lien au clavier est « Aller au contenu principal » (visible au focus).
- Les cadres de focus sont explicites et les thèmes clair, sombre et contraste élevé Windows restent lisibles.
- Éviter les annonces automatiques en continu pour les boutons/axes HID mis à jour très fréquemment.
- Le bouton « Annoncer les boutons actifs » lit l'état à la demande plutôt que de monopoliser le lecteur d'écran.
- Les tableaux ont des noms et des en-têtes ; les boutons répétés « Détails » sont distinguables.
- T14 : les commandes de vibrations, d'arrêt et les réglages d'intensité sont désactivés ; seule l'inspection HID sans écriture reste possible.

## Recette manuelle prioritaire (Windows 11)

Réaliser ces étapes sans toucher aux pilotes Bluetooth et **sans exécuter de test T14** :

- [x] **A01 — Lien d'évitement.** Ouvrir PadBridge et appuyer sur `Tab` une fois : le lien « Aller au contenu principal » doit devenir visible. Valider par `Entrée`, puis vérifier le focus sur le contenu.
- [x] **A02 — Navigation.** Avec `Tab`, `Maj+Tab` et `Entrée`, ouvrir les sept écrans ; chaque changement doit donner le focus au titre de l'écran.
- [ ] **A03 — Focus visible.** Un encadrement bien visible suit chaque bouton, lien, sélection, curseur et case à cocher.
- [ ] **A04 — Réglages.** Naviguer entre les champs de Paramètres et Profils au clavier. Modifier une zone morte avec les touches fléchées, puis vérifier la valeur affichée. Ne pas enregistrer de changement involontaire.
- [ ] **A05 — Appareils.** Dans la liste des appareils, chaque bouton « Détails » doit avoir un nom distinct et ouvrir le bon appareil.
- [x] **A06a — Lecture à la demande.** Le bouton « Annoncer les boutons actifs » est utilisable et affiche l'état à la demande (retour utilisateur, 20/10/2026).
- [ ] **A06 — Tests manette.** Le schéma et le texte « Boutons actifs » reflètent les pressions. Un lecteur d'écran ne doit pas annoncer chaque échantillon HID ; le bouton « Annoncer les boutons actifs » fournit un état à la demande.
- [ ] **A07 — Erreurs.** Provoquer une erreur bénigne de formulaire (nom de profil vide, sans sauvegarde) puis vérifier que le message est lisible en texte et non seulement en couleur.
- [ ] **A08 — Export.** Dans Diagnostics, ouvrir l'aperçu, parcourir le texte au clavier, puis confirmer que l'export n'est possible qu'après consentement. Annuler sans exporter.
- [ ] **A09 — Thèmes.** Comparer clair, sombre et « suivre le système ». Activer temporairement le contraste élevé Windows ; le focus, les badges et les boutons actifs doivent rester discernables.
- [ ] **A10 — Zoom.** Avec l'agrandissement Windows ou le zoom de WebView si disponible, vérifier l'absence de perte d'actions essentielles. La navigation doit pouvoir s'afficher horizontalement sur une largeur réduite.
- [ ] **A11 — T14.** Confirmer que les commandes de vibration et d'arrêt sont désactivées, mais que l'inspection « sans vibration » est accessible.
- [ ] **A12 — Quitter.** Quitter PadBridge par la zone de notification, puis confirmer que la manette répond dans `joy.cpl`.

**Retour de recette — 20 octobre 2026 :** l'utilisateur a confirmé « ça marche » après la consigne de vérifier le saut au contenu, le focus lors de la navigation et le bouton d'annonce des touches. Les autres scénarios ci-dessus n'ont pas été vérifiés explicitement ; l'efficacité des annonces avec un lecteur d'écran n'est pas encore établie.

## Points non démontrés par les tests automatiques

- Les tests TypeScript et Rust/Windows vérifient la compilation, **pas** le parcours clavier réel ni les annonces vocales.
- Contrastes de tous les états à mesurer et contrôler au regard du WCAG 2.2 AA ; l'estimation visuelle ne suffit pas.
- Tester réellement avec Narrateur Windows ou NVDA pour éviter les régressions liées à WebView2.
- Tester le zoom et la réorganisation visuelle sur une fenêtre redimensionnée.
- Ajouter ultérieurement une automatisation de vérification statique (axe-core ou équivalent) si la plateforme de test le permet.

## Critères de clôture

PB-024 ne sera marqué **validé sur matériel** qu'après retour explicite sur la navigation au clavier, le focus visible, la lecture des contenus importants, les erreurs et le contraste. Aucune affirmation de conformité WCAG complète sans audit dédié.
