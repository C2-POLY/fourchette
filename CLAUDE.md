# Fourchette

A Git client for macOS in Rust and egui: the history graph, branches, worktrees, stashes, an undo for every local action, a Claude assistant, the repository switcher and the GitHub project of the open repository. It began as a rebuild of Fork with a look closer to GitKraken.

## Project state

- `app/`: the application (the only code). `app/MECHANICS.md` and `app/mechanics.json` hold the values measured in the original app, with their evidence ids; keep them in sync when one of those values changes.
- `investigation.cany/`, `capture/`, `craft.json`, `attachments/`: what the rebuild tool found in the original app. Read them, never edit them by hand.
- Git ignores the tool's working state (`feed.json`, `usage.json`, `build-activity.log`, `inbox/`, `pinned/`, `checks/`) and `app/target/`.

## Stack

Rust 2021 (rust-version 1.85), eframe and egui 0.32, `rfd` for file dialogs, `serde_json`, `image`. Git, Claude Code and GitHub are reached through their command lines (`git`, `claude`, `gh`), never through a library or a token kept by the app. One exception: the update check reads the public releases API and downloads the .dmg with `curl` (no `gh`, which is optional for people who use the app, and no token).

## Commands

Run them from `app/`.

```bash
# Build and run on a repository (no argument: the current folder if it is a repository, else the last one opened)
cargo run -- /path/to/repo

# Unit tests (fast, offline)
cargo test

# Live and slow tests: they call Claude Code and GitHub for real, and install an update from a disk image made on the spot
cargo test -- --ignored --nocapture

# Lints: a change adds no warning to the 5 that main has (dead code in git.rs twice, three style hints in palette.rs, git.rs and library.rs)
cargo clippy

# Screenshot of the sample project (run from a folder that is not a repository, such as /tmp)
(cd /tmp && <app>/target/debug/fourchette --screenshot <out.png>)
```

Release (macOS), from `app/`:

```bash
# Build the universal .app and .dmg into target/release-bundle/
scripts/release.sh

# Create or update the GitHub pre-release v<version> (clean main only)
scripts/release.sh --publish

# Re-render the .dmg window background after changing assets/dmg/background.svg (needs Chrome)
scripts/dmg-background.sh
```

The .dmg is laid out by `dmgbuild` (installed by `release.sh` in `target/dmgbuild`), from `scripts/dmg-settings.py`: the background, the window size and where the two icons sit.

Screenshot views, set before `--screenshot`: `FOURCHETTE_PALETTE="<query>"` opens the palette, `FOURCHETTE_SWITCHER=1` the repository switcher, `FOURCHETTE_SETTINGS=1` the Settings window (`FOURCHETTE_SETTINGS=<section>` at that section: `appearance`, `fetch`), `FOURCHETTE_APPEARANCE=dark|light|system` shows a mode without saving it, `FOURCHETTE_VARIANT=classic|gold|neon|forest` a variant, `FOURCHETTE_FONTS=<ui>,<code>` (for example `sf,menlo`; keys `inter|sf|helvetica|avenir` and `jetbrains|sfmono|menlo|monaco`) the fonts, `FOURCHETTE_CLEANUP_PREVIEW=1` the branch clean-up window (`=loading` while Claude reads), `FOURCHETTE_DIALOG=<name>` a dialog on the sample project (`confirm` deletes a tag, `rebase`, `branch`, `tag`, `delete-branch`, `delete-worktree`, `group`), `FOURCHETTE_MENU=<x>,<y>` right-clicks there in points to show a context menu (`70,281` a branch of the sidebar, `600,232` a commit), `FOURCHETTE_TOAST=<message>` a toast (`error:<message>` an error's), `FOURCHETTE_BOARD=1` the project board (`FOURCHETTE_BOARD=all` with the whole project), `FOURCHETTE_BOARD_DETAIL=<n>` the board with card #n's drawer open, `FOURCHETTE_CHECKOUT_SYNC=1` the Check out dialog of a branch behind its upstream (`FOURCHETTE_CHECKOUT_SYNC=diverged` one that has diverged). `FOURCHETTE_TABS=<n>` shows the sample project in n tabs, so the tab strip shows. `FOURCHETTE_UPDATE=1` shows the toolbar's "Update available" pill with a sample release, `FOURCHETTE_UPDATE=dialog` also opens the update dialog (`=downloading`, `=installing`, `=error` at that step). The window is 1360x840 points (2720x1680 pixels on a Retina screen).

## Architecture

- `main.rs`: arguments (`<repo>`, `--screenshot <png>`).
- `app.rs`: the state (`App`), the actions (`Cmd`, run once per frame by `App::run`) and the dialogs (`Dialog`). The views push `Cmd`s, never change the repository themselves.
- `app/views.rs` (toolbar, sidebar, history, details, diff, dialogs, keys), `app/palette.rs` (Cmd+K), `app/repos.rs` (Cmd+O), `app/project.rs` (the Project section), `app/board.rs` (the full-screen project board, Cmd+B; Esc closes its drawer then the board), `app/tab.rs` (one open repository's state), `app/tabs.rs` (opening, closing and switching tabs, the strip shown from two tabs, the tabs kept in `repos.json`), `app/assistant.rs` (Claude).
- `git.rs`, `worktree.rs`, `undo.rs`: Git through its command line. Every local action takes a `Snapshot` first, so Cmd+Z takes it back.
- `checkout.rs`: checking out a branch behind its upstream: instant checkout or the Check out dialog (`plan`), its actions, and the Git sequence run as one undo step (`run`).
- `menu.rs`: the macOS menu bar (Fourchette, File, Edit, View, Repository, Window, Help, laid out like the original's): a tested model of titles, shortcuts and greyed-out rules, and the AppKit menu. A pick is queued and turned into the `Cmd` of its button in `App::raw_input_hook` (Edit items go to the focused text field). `build.rs` links `assets/Info.plist` into the binary so the menu says "Fourchette" outside an .app bundle.
- `modal.rs`: the frame every dialog shares (#91): backdrop, centred card with shadow and 14 pt corners, header (icon tile, title, status line with branch pills, cross), body, footer (hint with keycaps, Cancel then the main action on the right, red when destructive), three widths, Enter and Esc, the scale-and-fade appearing. Also the choice cards, check rows, text fields and notes inside dialogs, and the card, backdrop and animation that Settings, the palette, the switcher and the toasts use. Call `ui::menu_rows` first in a context menu.
- `titlebar.rs`: no title bar strip, like the original. The content fills the window, the toolbar is its top edge (dragging its empty background moves the window, a double-click zooms it), and on macOS the red, yellow and green buttons are moved onto the toolbar's centre line, with `INSET` left free for them. `--screenshot` shows the egui content only, without those buttons.
- `library.rs`: the repository list (`~/Library/Application Support/Fourchette/repos.json`): groups, favourites, last opened, the chosen GitHub project.
- `github.rs`: the GitHub project, read only, through `gh` in a background thread: the projects of the repository, then the project's items through `gh api graphql` (types, labels, fields, sub-issues, linked pull requests), shared by the sidebar section and the board.
- `update.rs`: updates from the GitHub releases (pre-releases included), with `curl`: checked at launch then every 6 hours, only when the app runs from an .app bundle (not under `cargo run`). A release is newer when its version is higher, or the same version with its tag on another commit than the build's (`build.rs` embeds it as `FOURCHETTE_COMMIT`). The install checks the .dmg's SHA-256, copies the app next to the installed one, swaps them by renaming and relaunches; `app/updater.rs` holds the toolbar pill, the dialog and the polling.
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
- After each merge into `main`, move the current version's tag to `main`, then re-run `scripts/release.sh --publish` to update its pre-release. Tags only ever point at `main`:

```bash
git fetch --tags
git tag -f v0.1.0 origin/main && git push -f origin v0.1.0
```

- History: v0.1.0 is the work of 2026-11-04 (the rebuild, the repository switcher and the Project section).

## Commits and language

Commit messages, PR titles and code are in English and follow Conventional Commits (`feat(switcher): …`, `fix(github): …`). Issues and talk with the dev are in French. No AI attribution in commits, PRs or issues.
