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
    badge: 'libre et gratuit · macOS',
    titleStart: 'Git, d’un coup d’œil.',
    titleAccent: 'Et chaque geste annulable.',
    lead:
      'Fourchette est un client Git natif pour macOS, écrit en Rust. Lisez votre historique en graphe, créez des branches, fusionnez et mettez de côté en un clic, et reprenez n’importe quelle action locale avec ⌘Z.',
    primary: 'Télécharger pour macOS',
    secondary: 'Voir sur GitHub',
    shotAlt:
      'La fenêtre de Fourchette : les branches dans la barre latérale, l’historique en graphe coloré et le détail du commit choisi à droite',
  },
  features: {
    title: 'Tout ce que vous faites chaque jour, à un clic',
    items: [
      {
        icon: 'graph',
        title: 'Un historique qui se lit',
        text: 'Chaque branche sur sa voie colorée, des fusions en courbes, des étiquettes pour les branches, les remotes et les tags, et les fichiers et le diff du commit à côté.',
      },
      {
        icon: 'undo',
        title: 'Tout s’annule',
        text: 'Avant chaque action locale, Fourchette note vos branches, tags, stashes et HEAD. ⌘Z les remet en place, jusqu’à 30 fois : un commit, une fusion, une branche supprimée.',
      },
      {
        icon: 'worktree',
        title: 'Les worktrees, gérés',
        text: 'Les agents comme Claude Code ouvrent beaucoup de worktrees. Fourchette les liste, et retire le worktree avant de supprimer sa branche, en gardant ses changements pour l’annulation.',
      },
      {
        icon: 'board',
        title: 'Votre projet GitHub en tableau',
        text: 'Les issues du dépôt ouvert dans leurs colonnes, avec leurs types, labels, sous-issues et pull requests liées. Glissez une carte dans une autre colonne, et GitHub suit.',
      },
      {
        icon: 'palette',
        title: 'Au clavier d’abord',
        text: '⌘K trouve une commande ou une branche, ⌘O change de dépôt, ⌘1 à ⌘9 ouvrent vos favoris, et chaque dépôt a son onglet.',
      },
      {
        icon: 'spark',
        title: 'Claude, avec votre abonnement',
        text: 'Écrire le message de commit d’après vos changements, expliquer un commit, trouver les branches qui peuvent partir. Par Claude Code : l’app ne garde aucune clé d’API.',
      },
    ],
  },
  showcase: [
    {
      kicker: 'GitHub Projects',
      title: 'Planifier et coder dans la même fenêtre',
      text: 'Ouvrez le projet lié au dépôt : une colonne par statut, des cartes avec leur type, leurs labels, leur priorité et leur avancement. Glissez une carte pour changer son statut ou sa place : le tableau bouge tout de suite et GitHub est mis à jour en arrière-plan. Le détail d’un ticket et une branche à son nom sont à un clic.',
      shot: 'board-drag',
      alt: 'Le tableau du projet : une carte soulevée du Backlog et glissée dans la colonne Ready, qui lui ouvre une place',
    },
    {
      kicker: 'Nettoyage des branches',
      title: 'Ranger ses branches sans crainte',
      text: 'Claude repère les branches fusionnées, celles qui gardent du travail et celles qui sont dans un worktree. Vous cochez celles à supprimer, et l’annulation les ramène toutes.',
      shot: 'cleanup',
      alt: 'La fenêtre de nettoyage : trois branches fusionnées cochées pour être supprimées, deux autres à revoir d’abord',
    },
    {
      kicker: 'Palette de commandes',
      title: 'Tapez ce que vous voulez faire',
      text: 'Pull, créer une branche, basculer ou fusionner : quelques lettres dans ⌘K suffisent, de n’importe où dans l’app.',
      shot: 'palette',
      alt: 'La palette de commandes, avec « br » tapé : des commandes, le nettoyage par Claude et des branches à ouvrir ou fusionner',
    },
    {
      kicker: 'À votre goût',
      title: 'Sombre, clair, vos polices',
      text: 'Suivez macOS ou choisissez un mode, prenez vos polices d’interface et de code, le modèle de Claude, et réglez comment Fourchette fait ses pull, ses fetch et ses diffs.',
      shot: 'settings',
      alt: 'La fenêtre Réglages : le mode d’apparence et les polices d’interface et de code',
    },
  ],
  teams: {
    title: 'Pensé pour les équipes qui livrent',
    items: [
      { title: 'Les issues à côté du code', text: 'Le tableau du projet lit GitHub avec votre propre connexion gh : ce qui est en cours, ce qui est en revue, qui s’en occupe.' },
      { title: 'Beaucoup de dépôts, bien rangés', text: 'Groupez vos dépôts, mettez vos favoris en étoile, ajoutez tout un dossier d’un coup, et gardez chacun ouvert dans son onglet.' },
      { title: 'Sûr par défaut', text: 'Chaque action locale peut être reprise, et rien ne part nulle part sans que vous le demandiez à Claude ou à GitHub.' },
    ],
  },
  build: {
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
    stepsTitle: 'Ou compilez-le depuis les sources, avec Rust 1.85 ou plus récent',
    copy: 'Copier',
    copied: 'Copié',
    note: 'Sans argument, il ouvre le dossier courant si c’est un dépôt, sinon le dernier que vous avez ouvert.',
  },
  open: {
    title: 'Code ouvert, feuille de route ouverte',
    text: 'Fourchette est un logiciel libre, sous licence MIT ou Apache 2.0, au choix. Lisez le code, ouvrez une issue, ou suivez ce qui arrive sur le tableau public du projet.',
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
