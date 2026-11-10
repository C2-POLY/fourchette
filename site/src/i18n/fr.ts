// Every string the French page shows. en.ts has the same keys.
const fr: typeof import('./en').default = {
  meta: {
    title: 'Fourchette : un client Git natif et rapide pour macOS, libre et gratuit',
    description:
      'Un client Git pour macOS écrit en Rust : un graphe d’historique lisible, une annulation pour chaque action locale, les worktrees, votre projet GitHub en tableau, et Claude pour écrire les messages de commit et ranger les branches.',
  },
  a11y: {
    skip: 'Aller au contenu',
    mainNav: 'Principale',
    theme: 'Basculer entre les thèmes sombre et clair',
    language: 'Langue',
  },
  nav: {
    features: 'Fonctions',
    teams: 'En équipe',
    build: 'L’obtenir',
    github: 'GitHub',
  },
  hero: {
    note: 'Libre et gratuit · macOS 11 ou plus récent · Apple Silicon et Intel',
    titleStart: 'Git, d’un coup d’œil.',
    titleEnd: 'Et chaque geste annulable.',
    lead: 'Un client Git natif pour macOS, écrit en Rust. Votre historique en graphe lisible, branches et stashes en un clic, et ⌘Z pour toute action locale.',
    primary: 'Télécharger pour macOS',
    secondary: 'Voir sur GitHub',
    shotAlt:
      'La fenêtre de Fourchette : les branches dans la barre latérale, l’historique en graphe coloré et le détail du commit choisi à droite',
  },
  features: {
    ref: 'log',
    title: 'Se lit comme votre historique',
    lead: 'Ce que Fourchette fait bien, un commit à la fois.',
    items: [
      {
        ref: 'graph',
        title: 'Un historique qui se lit',
        text: 'Chaque branche sur sa voie colorée, des fusions en courbes, des étiquettes pour les branches, les remotes et les tags, et les fichiers et le diff du commit à côté.',
      },
      {
        ref: 'worktrees',
        title: 'Les worktrees, gérés',
        text: 'Les agents comme Claude Code ouvrent beaucoup de worktrees. Fourchette les liste, et retire le worktree avant de supprimer sa branche, en gardant ses changements pour l’annulation.',
      },
      {
        ref: 'project',
        title: 'Votre projet GitHub en tableau',
        text: 'Les issues du dépôt ouvert dans leurs colonnes, avec leurs types, labels, sous-issues et pull requests liées. Glissez une carte, et GitHub suit.',
      },
      {
        ref: 'keyboard',
        title: 'Au clavier d’abord',
        text: '⌘K trouve une commande ou une branche, ⌘O change de dépôt, ⌘1 à ⌘9 ouvrent vos favoris, et chaque dépôt a son onglet.',
      },
      {
        ref: 'claude',
        title: 'Claude, avec votre abonnement',
        text: 'Écrire le message de commit d’après vos changements, expliquer un commit, trouver les branches qui peuvent partir. Par Claude Code : l’app ne garde aucune clé d’API.',
      },
    ],
  },
  undo: {
    ref: 'undo',
    title: 'Allez-y. ⌘Z est juste là.',
    text: 'Avant chaque action locale, Fourchette note vos branches, tags, stashes et HEAD. ⌘Z les remet en place, jusqu’à 30 fois : un commit, une fusion, une branche supprimée.',
    try: 'Essayez sur ce petit historique. Votre propre ⌘Z marche aussi.',
    label: 'Un historique d’exemple pour essayer l’annulation',
    commit: 'Commit',
    delete: 'Supprimer la branche',
    stash: 'Stash',
    undo: 'Annuler',
    took: 'Repris',
    steps: 'étapes à annuler',
    none: 'Plus rien à annuler',
    changes: 'Changements non commités',
  },
  showcase: {
    board: {
      ref: 'GitHub Projects',
      title: 'Planifier et coder dans la même fenêtre',
      text: 'Une colonne par statut, des cartes avec leur type, leurs labels, leur priorité et leur avancement. Glissez une carte pour changer son statut ou sa place : le tableau bouge tout de suite et GitHub est mis à jour en arrière-plan.',
      alt: 'Le tableau du projet : une carte soulevée du Backlog et glissée dans la colonne Ready, qui lui ouvre une place',
    },
    city: {
      ref: 'Vue City',
      title: 'Puis regardez-le se construire',
      text: 'Passez le tableau en City et chaque ticket devient une parcelle : un terrain nu au Backlog, des fondations sous une grue qui tourne une fois en cours, un bâtiment fini quand c’est fait. Chaque pull request ouverte roule vers son chantier en camion, de la couleur de ses checks.',
      alt: 'Le tableau en ville isométrique : des quartiers de styles différents, des bâtiments à chaque étape, des grues et des camions entre eux',
    },
    home: {
      ref: 'Page d’accueil',
      title: 'Tous vos dépôts, à un onglet',
      text: 'Un nouvel onglet s’ouvre sur vos dépôts, groupés par organisation GitHub, favoris et récents d’abord. Clonez-en un depuis votre compte en un clic, et retirez ceux dont le dossier a disparu, avec une annulation.',
      alt: 'La page d’accueil : les dépôts en sections par organisation, avec un champ de recherche et le bouton Cloner',
    },
    cleanup: {
      ref: 'Nettoyage des branches',
      title: 'Ranger ses branches sans crainte',
      text: 'Claude repère les branches fusionnées, celles qui gardent du travail et celles qui sont dans un worktree. Vous cochez celles à supprimer, et l’annulation les ramène toutes.',
      alt: 'La fenêtre de nettoyage : trois branches fusionnées cochées pour être supprimées, deux autres à revoir d’abord',
    },
    palette: {
      ref: '⌘K',
      title: 'Tapez ce que vous voulez faire',
      text: 'Pull, créer une branche, basculer ou fusionner : quelques lettres suffisent, de n’importe où dans l’app.',
      alt: 'La palette de commandes, avec « br » tapé : des commandes, le nettoyage par Claude et des branches à ouvrir ou fusionner',
    },
    settings: {
      ref: 'Réglages',
      title: 'Sombre, clair, vos polices, votre couleur',
      text: 'Suivez macOS ou choisissez un mode, prenez vos polices d’interface et de code et la couleur de l’icône dans le Dock. Et pour les jours plus calmes, un skin gamer désordonné : chips et canettes s’entassent là où vous traînez, jusqu’à ce que vous preniez le robot aspirateur.',
      alt: 'La fenêtre Réglages : le mode d’apparence, le skin, la couleur de l’icône et les polices d’interface et de code',
    },
  },
  companion: {
    ref: 'iPhone et iPad',
    title: 'Votre tableau dans la poche',
    text: 'L’app compagnon lit GitHub avec votre propre connexion : le tableau du projet de chaque dépôt, ses pull requests avec leurs checks, et ses branches avec leur historique. Déplacez une carte par son menu, d’un balayage, ou d’un glisser sur iPad.',
    note: 'Pas encore sur l’App Store : compilez-la avec Xcode depuis le dossier ios du dépôt.',
    shots: [
      { name: 'board', alt: 'Le compagnon sur iPhone : le tableau du projet d’un dépôt, une colonne par statut' },
      { name: 'pulls', alt: 'Le compagnon sur iPhone : les pull requests ouvertes avec l’état de leurs checks' },
      { name: 'history', alt: 'Le compagnon sur iPhone : une branche et ses derniers commits' },
    ],
  },
  teams: {
    ref: 'team',
    title: 'En équipe, rien ne se perd',
    items: [
      { key: 'board', title: 'Les issues à côté du code', text: 'Le tableau du projet lit GitHub avec votre propre connexion gh : ce qui est en cours, ce qui est en revue, qui s’en occupe.' },
      { key: 'repos', title: 'Beaucoup de dépôts, bien rangés', text: 'Groupez vos dépôts, mettez vos favoris en étoile, ajoutez tout un dossier d’un coup ou clonez depuis GitHub, et gardez chacun ouvert dans son onglet.' },
      { key: 'safety', title: 'Sûr par défaut', text: 'Chaque action locale peut être reprise, et rien ne part nulle part sans que vous le demandiez à Claude ou à GitHub.' },
    ],
  },
  build: {
    ref: 'release',
    title: 'Obtenir Fourchette',
    lead: 'Téléchargez l’image disque depuis les Releases GitHub. Fourchette vous prévient ensuite quand une nouvelle version sort, et l’installe en un clic.',
    download: 'Télécharger la dernière version',
    installTitle: 'Installer',
    install: [
      'Ouvrez Fourchette-<version>.dmg et glissez Fourchette dans Applications.',
      'Cette version n’est pas encore notarisée, donc macOS bloque la première ouverture : dans Réglages Système › Confidentialité et sécurité, cliquez sur « Ouvrir quand même ».',
    ],
    needsTitle: 'Il vous faut',
    needs: [
      'macOS 11 ou plus récent, sur Apple Silicon ou Intel',
      'Git (les outils en ligne de commande de Xcode)',
      'En option : la CLI GitHub (gh) pour le tableau du projet, et Claude Code pour l’assistant',
    ],
    stepsTitle: 'Ou compilez-le depuis les sources, avec Rust 1.95 ou plus récent',
    copy: 'Copier',
    copied: 'Copié',
    note: 'Sans argument, il ouvre le dossier courant si c’est un dépôt, sinon les onglets laissés ouverts, sinon une page d’accueil avec vos dépôts.',
  },
  open: {
    ref: 'HEAD',
    title: 'Code ouvert, feuille de route ouverte',
    text: 'Logiciel libre, sous licence MIT ou Apache 2.0, au choix. Lisez le code, ouvrez une issue, ou suivez ce qui arrive sur le tableau public du projet.',
    code: 'Code source',
    issues: 'Issues',
    roadmap: 'Feuille de route',
  },
  footer: {
    licence: 'MIT OR Apache-2.0',
    by: 'Fait par C2-POLY',
  },
};

export default fr;
