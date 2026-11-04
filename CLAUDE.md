# Fourchette

A Git client for macOS in Rust and egui: the history graph, branches, worktrees, stashes, an undo for every local action, a Claude assistant, the repository switcher and the GitHub project of the open repository. It began as a rebuild of Fork with a look closer to GitKraken.

## Project state

- `app/`: the application (the only code). `app/MECHANICS.md` and `app/mechanics.json` hold the values measured in the original app, with their evidence ids; keep them in sync when one of those values changes.
- `investigation.cany/`, `capture/`, `craft.json`, `attachments/`: what the rebuild tool found in the original app. Read them, never edit them by hand.
- Git ignores the tool's working state (`feed.json`, `usage.json`, `build-activity.log`, `inbox/`, `pinned/`, `checks/`) and `app/target/`.

## Stack

Rust 2021 (rust-version 1.85), eframe and egui 0.32, `rfd` for file dialogs, `serde_json`, `image`. Git, Claude Code and GitHub are reached through their command lines (`git`, `claude`, `gh`), never through a library or a token kept by the app.

## Commands

Run them from `app/`.

```bash
# Build and run on a repository (no argument: the current folder if it is a repository, else the last one opened)
cargo run -- /path/to/repo

# Unit tests (fast, offline)
cargo test

# Live tests: they call Claude Code and GitHub for real
cargo test -- --ignored --nocapture

# Lints: a change adds no warning to the 5 that main has (dead code in git.rs and app.rs, two style hints)
cargo clippy

# Screenshot of the sample project (run from a folder that is not a repository, such as /tmp)
(cd /tmp && <app>/target/debug/fourchette --screenshot <out.png>)
```

Screenshot views, set before `--screenshot`: `FOURCHETTE_PALETTE="<query>"` opens the palette, `FOURCHETTE_SWITCHER=1` the repository switcher, `FOURCHETTE_SETTINGS=1` the Settings window (`FOURCHETTE_SETTINGS=<section>` at that section: `appearance`, `fetch`), `FOURCHETTE_APPEARANCE=dark|light|system` shows a mode without saving it, `FOURCHETTE_VARIANT=classic|gold|neon|forest` a variant, `FOURCHETTE_CLEANUP_PREVIEW=1` the branch clean-up window, `FOURCHETTE_BOARD=1` the project board (`FOURCHETTE_BOARD=all` with the whole project), `FOURCHETTE_BOARD_DETAIL=<n>` the board with card #n's drawer open. The window is 1360x840 points (2720x1680 pixels on a Retina screen).

## Architecture

- `main.rs`: arguments (`<repo>`, `--screenshot <png>`).
- `app.rs`: the state (`App`), the actions (`Cmd`, run once per frame by `App::run`) and the dialogs (`Dialog`). The views push `Cmd`s, never change the repository themselves.
- `app/views.rs` (toolbar, sidebar, history, details, diff, dialogs, keys), `app/palette.rs` (Cmd+K), `app/repos.rs` (Cmd+O), `app/project.rs` (the Project section), `app/board.rs` (the full-screen project board, Cmd+B; Esc closes its drawer then the board), `app/assistant.rs` (Claude).
- `git.rs`, `worktree.rs`, `undo.rs`: Git through its command line. Every local action takes a `Snapshot` first, so Cmd+Z takes it back.
- `menu.rs`: the macOS menu bar (Fourchette, File, Edit, View, Repository, Window, Help, laid out like the original's): a tested model of titles, shortcuts and greyed-out rules, and the AppKit menu. A pick is queued and turned into the `Cmd` of its button in `App::raw_input_hook` (Edit items go to the focused text field). `build.rs` links `assets/Info.plist` into the binary so the menu says "Fourchette" outside an .app bundle.
- `titlebar.rs`: no title bar strip, like the original. The content fills the window, the toolbar is its top edge (dragging its empty background moves the window, a double-click zooms it), and on macOS the red, yellow and green buttons are moved onto the toolbar's centre line, with `INSET` left free for them. `--screenshot` shows the egui content only, without those buttons.
- `library.rs`: the repository list (`~/Library/Application Support/Fourchette/repos.json`): groups, favourites, last opened, the chosen GitHub project.
- `github.rs`: the GitHub project, read only, through `gh` in a background thread: the projects of the repository, then the project's items through `gh api graphql` (types, labels, fields, sub-issues, linked pull requests), shared by the sidebar section and the board.
- `ai.rs`: Claude Code through the person's subscription (`claude -p`, no tools, API keys hidden).
- `graph.rs`, `ui.rs`, `theme.rs`: the graph layout, drawn widgets and colours.
- Network and AI work never block the window: a thread and a channel polled each frame (`App::spawn`, `ai_poll`, `project_poll`).
- The app's font has no emoji or symbol beyond the basic ones: draw an icon with the painter rather than typing a glyph, and check it on a screenshot.

## New subjects

Every new subject (feature, improvement or bug fix) follows the `topic` skill, in `.claude/skills/topic/SKILL.md`, also run with `/topic`: analyse, find or define the GitHub issue with the dev, make a worktree (always, without asking), work, test (cargo test, clippy, screenshots), have the dev try it from their terminal, then commit, push, PR, move the issue on the project and tidy up. One subject per issue, worktree and PR. No code before the issue is defined.

## Work items (GitHub)

Work items are GitHub issues of `C2-POLY/fourchette`, tracked in the organisation's project **Fourchette** (<https://github.com/orgs/C2-POLY/projects/1>, number 1). `gh` must be signed in with the `project` scope (`gh auth refresh -s project`).

Issues are written in French and kept short:

- Title: `<Zone> : <ce qu'il apporte>`, for example `Sélecteur : ranger un dépôt dans un groupe`.
- Body: `**En tant que** …, **je veux** …, **afin de** …`, then `**Critères d'acceptation**` with 3 to 5 checkboxes, then `**Livré** : \`vX.Y.Z\`` once delivered.
- Type: the organisation's issue types, `Feature` (a capability, a parent issue when it is split), `Task` (an improvement, tooling, docs) or `Bug`. Split work is sub-issues; "B cannot start before A" is a `blocked by` relation, not a sentence.
- Labels: one `zone:` label (`zone:graphe`, `zone:historique`, `zone:barre-latérale`, `zone:sélecteur`, `zone:github`, `zone:assistant`, `zone:git`, `zone:annuler`, `zone:worktrees`, `zone:palette`, `zone:thème`, `zone:outillage`), and once delivered one version label (`v0.2.0`). Create a missing label rather than skipping it.
- Assignee: the person who did the work.
- Status on the project: `Backlog` (defined), `Ready` (ready to take), `In progress` (its worktree exists), `In review` (PR open), `To test` (merged, a criterion still untested by hand), `Done` (tested by the dev and merged).
- Every delivered change has an issue, including small ones only visible in `git log`.
- The PR closes its issue (`Closes #12` in its body).

Project ids for `gh project item-edit`: project `PVT_kwDODd-WqM4BqKPF`, Status field `PVTSSF_lADODd-WqM4BqKPFzhoijpg`, options Backlog `d1563a06`, Ready `4e73c320`, In progress `35343aff`, In review `5334ffc6`, To test `bc91ddaa`, Done `3f2a2aea`.

## Versioning

- The version lives only in `app/Cargo.toml` (`version`), as `MAJOR.MINOR.PATCH`.
- One working day is one minor version (0.x while the app is not distributed). Bump it in the first commit of the day that changes something visible in the app, inside that commit. A day that only fixes bugs bumps the patch instead. Commits that only touch docs or tooling do not bump it.
- Never put the version number in a commit message.
- After each merge into `main`, move the current version's tag to `main`. Tags only ever point at `main`:

```bash
git fetch --tags
git tag -f v0.1.0 origin/main && git push -f origin v0.1.0
```

- History: v0.1.0 is the work of 2026-11-04 (the rebuild, the repository switcher and the Project section).

## Commits and language

Commit messages, PR titles and code are in English and follow Conventional Commits (`feat(switcher): …`, `fix(github): …`). Issues and talk with the dev are in French. No AI attribution in commits, PRs or issues.
