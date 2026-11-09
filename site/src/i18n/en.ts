// Every string the English page shows. fr.ts has the same keys.
const en = {
  meta: {
    title: 'Fourchette: a fast, native Git client for macOS, free and open source',
    description:
      'A Git client for macOS written in Rust: a clear history graph, an undo for every local action, worktrees, your GitHub project as a board, and Claude to write commit messages and tidy branches.',
  },
  a11y: {
    skip: 'Skip to content',
    mainNav: 'Main',
    theme: 'Switch between the dark and light themes',
    language: 'Language',
  },
  nav: {
    features: 'Features',
    teams: 'For teams',
    build: 'Get it',
    github: 'GitHub',
  },
  hero: {
    badge: 'free and open source · macOS',
    titleStart: 'Git, at a glance.',
    titleAccent: 'And every step undoable.',
    lead:
      'Fourchette is a native Git client for macOS, written in Rust. Read your history as a graph, branch, merge and stash in a click, and take any local action back with ⌘Z.',
    primary: 'Download for macOS',
    secondary: 'View on GitHub',
    shotAlt:
      'The Fourchette window: branches in the sidebar, the history as a coloured graph, and the selected commit’s details on the right',
  },
  features: {
    title: 'Everything you do every day, one click away',
    items: [
      {
        icon: 'graph',
        title: 'A history you can read',
        text: 'Every branch on its own coloured lane, curved merges, labels for branches, remotes and tags, and the commit’s files and diff beside it.',
      },
      {
        icon: 'undo',
        title: 'Undo for everything',
        text: 'Before each local action, Fourchette notes your branches, tags, stashes and HEAD. ⌘Z puts them back, up to 30 steps: a commit, a merge, a deleted branch.',
      },
      {
        icon: 'worktree',
        title: 'Worktrees, handled',
        text: 'Agents like Claude Code open many worktrees. Fourchette lists them, and removes a worktree before deleting its branch, keeping its changes for Undo.',
      },
      {
        icon: 'board',
        title: 'Your GitHub project as a board',
        text: 'The issues of the open repository in their columns, with types, labels, sub-issues and linked pull requests. Drag a card to another column and GitHub follows.',
      },
      {
        icon: 'palette',
        title: 'Keyboard first',
        text: '⌘K finds any command or branch, ⌘O switches repository, ⌘1 to ⌘9 open your favourites, and each repository gets its own tab.',
      },
      {
        icon: 'spark',
        title: 'Claude, through your subscription',
        text: 'Write the commit message from your changes, explain a commit, or find the branches that can go. Through Claude Code: no API key kept by the app.',
      },
    ],
  },
  showcase: [
    {
      kicker: 'GitHub Projects',
      title: 'Plan and code in the same window',
      text: 'Open the project linked to the repository: one column per status, cards with their type, labels, priority and progress. Drag a card to change its status or its place: the board moves at once and GitHub is updated in the background. A ticket’s details and a branch named after it are a click away.',
      shot: 'board-drag',
      alt: 'The project board: a card lifted from Backlog and dragged into the Ready column, which opens a gap for it',
    },
    {
      kicker: 'City view',
      title: 'Watch your project being built',
      text: 'Switch the board to City and every ticket becomes a plot: bare ground in Backlog, foundations under a turning crane once in progress, a finished building when done. Each zone is a district with its own architecture, from the Mediterranean to Japan, and each open pull request drives to its site as a lorry in the colour of its checks.',
      shot: 'city',
      alt: 'The board as an isometric city: districts of different styles, buildings at every stage, cranes and lorries between them',
    },
    {
      kicker: 'Home page',
      title: 'All your repositories, one tab away',
      text: 'A new tab opens on your repositories, grouped by GitHub organisation, with your favourites and recents first. Clone one from your GitHub account in a click, and clear out the ones whose folder is gone, with an Undo.',
      shot: 'home',
      alt: 'The home page: repositories in sections by organisation, with a search field and the Clone button',
    },
    {
      kicker: 'Branch clean-up',
      title: 'Tidy branches without fear',
      text: 'Claude reads which branches are merged, which still hold work, and which sit in a worktree. You tick the ones to delete, and Undo brings them all back.',
      shot: 'cleanup',
      alt: 'The branch clean-up window: three merged branches ticked for deletion, two others to review first',
    },
    {
      kicker: 'Command palette',
      title: 'Type what you want to do',
      text: 'Pull, create a branch, check out or merge: a few letters in ⌘K are enough, from anywhere in the app.',
      shot: 'palette',
      alt: 'The command palette, with “br” typed: commands, the Claude clean-up and branches to check out or merge',
    },
    {
      kicker: 'Yours',
      title: 'Dark, light, your fonts, your colour',
      text: 'Follow macOS or pick a mode, choose your interface and code fonts and the colour of the app icon in the Dock and the Finder, Claude’s model, and how Fourchette pulls, fetches and shows diffs. And for slower days, a messy gamer skin: crisps and cans pile up where you linger, until you pick up the robot vacuum.',
      shot: 'settings',
      alt: 'The Settings window: appearance mode, skin, app icon colour, and interface and code fonts',
    },
  ],
  companion: {
    kicker: 'iPhone and iPad',
    title: 'Your board in your pocket',
    text: 'The companion app reads GitHub with your own sign-in: each repository’s project board, its pull requests with their checks, and its branches with their history. Move a card from your phone, by its menu, a swipe or a drag on iPad.',
    note: 'Not on the App Store yet: build it from the ios folder of the repository with Xcode.',
    shots: [
      { name: 'board', alt: 'The companion on iPhone: a repository’s project board, one column per status' },
      { name: 'pulls', alt: 'The companion on iPhone: open pull requests with the state of their checks' },
      { name: 'history', alt: 'The companion on iPhone: a branch and its recent commits' },
    ],
  },
  teams: {
    title: 'Built for teams that ship',
    items: [
      { title: 'Issues next to the code', text: 'The project board reads GitHub through your own gh sign-in: what is in progress, what is in review, who is on it.' },
      { title: 'Many repositories, tidy', text: 'Group repositories, star favourites, add a whole folder at once or clone from GitHub, and keep each one open in its tab.' },
      { title: 'Safe by default', text: 'Every local action can be taken back, and nothing is sent anywhere unless you ask Claude or GitHub for it.' },
    ],
  },
  build: {
    title: 'Get Fourchette',
    lead: 'Download the disk image from GitHub Releases. Fourchette then tells you when a new version is out and installs it in one click.',
    download: 'Download the latest version',
    installTitle: 'Install',
    install: [
      'Open Fourchette-<version>.dmg and drag Fourchette to Applications.',
      'The build is not notarized yet, so macOS blocks the first open: in System Settings › Privacy & Security, click “Open Anyway”.',
    ],
    needsTitle: 'You need',
    needs: [
      'macOS 11 or later, on Apple Silicon or Intel',
      'Git (the Xcode Command Line Tools)',
      'Optional: the GitHub CLI (gh) for the project board, and Claude Code for the assistant',
    ],
    stepsTitle: 'Or build it from source, with Rust 1.95 or later',
    copy: 'Copy',
    copied: 'Copied',
    note: 'Without an argument, it opens the current folder if it is a repository, else the tabs you left open, else a home page with your repositories.',
  },
  open: {
    title: 'Open source, open roadmap',
    text: 'Fourchette is free software under the MIT or Apache 2.0 licence, at your choice. Read the code, open an issue, or follow what is coming on the public project board.',
    code: 'Source code',
    issues: 'Issues',
    roadmap: 'Roadmap',
  },
  footer: {
    licence: 'MIT OR Apache-2.0',
    by: 'Made by C2-POLY',
  },
};

export default en;
