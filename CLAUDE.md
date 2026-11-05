# Fourchette

A Git client for macOS in Rust and egui: the history graph, branches, worktrees, stashes, an undo for every local action, a Claude assistant, the repository switcher and the GitHub project of the open repository. It began as a rebuild of Fork with a look closer to GitKraken.

## Project state

- `app/`: the application (the only code). `app/MECHANICS.md` and `app/mechanics.json` hold the values measured in the original app, with their evidence ids; keep them in sync when one of those values changes.
- `investigation.cany/`, `capture/`, `craft.json`, `attachments/`: what the rebuild tool found in the original app. Read them, never edit them by hand.
- `site/`: the website, an Astro project published on GitHub Pages (<https://c2-poly.github.io/fourchette/>) by `.github/workflows/site.yml` on every push to `main` that touches it. English and French strings live in `site/src/i18n/`; its screenshots in `site/src/assets/shots/` come from the app's screenshot mode, in dark and light. Preview it with `npm install && npm run dev` from `site/`.
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

# Lints: no warning at all, so any new one stands out
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

Screenshot views, set before `--screenshot`: `FOURCHETTE_PALETTE="<query>"` opens the palette, `FOURCHETTE_SWITCHER=1` the repository switcher, `FOURCHETTE_SETTINGS=1` the Settings window (`FOURCHETTE_SETTINGS=<section>` at that section: `appearance`, `fetch`), `FOURCHETTE_APPEARANCE=dark|light|system` shows a mode without saving it, `FOURCHETTE_VARIANT=classic|gold|neon|forest` a variant, `FOURCHETTE_FONTS=<ui>,<code>` (for example `sf,menlo`; keys `inter|sf|helvetica|avenir` and `jetbrains|sfmono|menlo|monaco`) the fonts, `FOURCHETTE_CLEANUP_PREVIEW=1` the branch clean-up window (`=loading` while Claude reads), `FOURCHETTE_CHANGELOG=<developers|product|leadership|users>` the changelog window with the sample changelog, `FOURCHETTE_RESOLVE=1` the conflict window with sample proposals, `FOURCHETTE_PR=1` the pull request window with the sample draft, `FOURCHETTE_DIALOG=<name>` a dialog on the sample project (`confirm` deletes a tag, `rebase`, `branch`, `tag`, `delete-branch`, `delete-worktree`, `group`, `pull` the Pull dialog of `develop` diverged from origin/develop, `merge` with conflicts foreseen, `merge-clean`, `merge-ff` where a fast-forward is possible, `merge-noff` with "No fast-forward" ticked, `merge-checking`, `merge-old` with a Git too old to check, `push`, `abort` the Abort confirmation of `FOURCHETTE_OPERATION`'s operation, else a merge), `FOURCHETTE_OPERATION=<merge|rebase|cherry-pick|revert>` the banner of an operation stopped on conflicts (`,resolved` appended: with all of them resolved), `FOURCHETTE_MENU=<x>,<y>` right-clicks there in points to show a context menu (`70,281` a branch of the sidebar, `600,232` a commit), `FOURCHETTE_TOAST=<message>` a toast (`error:<message>` an error's, `undo:<message>` one that offers Cmd+Z with its countdown bar), `FOURCHETTE_BOARD=1` the project board (`FOURCHETTE_BOARD=all` with the whole project), `FOURCHETTE_BOARD_LIST=1` the board as a list, `FOURCHETTE_BOARD_TREE=parents|all|nested` the board with only the top-level tickets, all of them, or each sub-issue inside its parent, `FOURCHETTE_BOARD_DETAIL=<n>` the board with card #n's drawer open, `FOURCHETTE_BOARD_DRAG=<n>` the board with card #n lifted mid-drag over the next column, `FOURCHETTE_CHECKOUT_SYNC=1` the Check out dialog of a branch that has diverged from its upstream (one only behind is fast-forwarded without asking). `FOURCHETTE_CHECKOUT_ACTION=ff|merge|rebase|reset|stay` selects that action, `FOURCHETTE_CHECKOUT_VIEW=now` the graph's "Now" view instead of "After" (the Pull dialog's too), `FOURCHETTE_CHECKOUT_MERGE=squash` Merge with that style, `FOURCHETTE_CHECKOUT_CLEAN=1` the dialog without local changes to stash. `FOURCHETTE_DIALOG=push` opens the Push dialog on the sample `develop`, 3 commits ahead of origin/develop (`push-diverged` 2 ahead and 3 behind, `push-force` the same with Force push asking to confirm, `push-new` to a new remote branch with all tags); `FOURCHETTE_PUSH_VIEW=now` shows its "Now" view instead of "After". `FOURCHETTE_ISSUE=<n>` puts the sample project on branch `feat/<n>-linked-issue`, so the toolbar shows issue #n's chip (`52` is Ready, `46` In progress); `FOURCHETTE_ISSUE=<n>,progress` adds the offer to move it to In progress, `,review` to In review, `,moved` the confirmation with its Undo. `FOURCHETTE_TABS=<n>` shows the sample project in n tabs, so the tab strip shows. `FOURCHETTE_UPDATE=1` shows the toolbar's "Update available" pill with a sample release, `FOURCHETTE_UPDATE=dialog` also opens the update dialog (`=downloading`, `=installing`, `=error` at that step). The window is 1360x840 points (2720x1680 pixels on a Retina screen).

## Architecture

- `main.rs`: arguments (`<repo>`, `--screenshot <png>`).
- `app.rs`: the state (`App`), the actions (`Cmd`, run once per frame by `App::run`) and the dialogs (`Dialog`). The views push `Cmd`s, never change the repository themselves.
- `app/views.rs` (toolbar, sidebar, history, details, diff, dialogs, keys), `app/palette.rs` (Cmd+K), `app/repos.rs` (Cmd+O), `app/project.rs` (the Project section), `app/board.rs` (the full-screen project board, Cmd+B; Esc closes its drawer then the board), `app/tab.rs` (one open repository's state), `app/tabs.rs` (opening, closing and switching tabs, the strip shown from two tabs, the tabs kept in `repos.json`), `app/assistant.rs` (Claude).
- `git.rs`, `worktree.rs`, `undo.rs`: Git through its command line. Every local action takes a `Snapshot` first, so Cmd+Z takes it back.
- `checkout.rs`: checking out a branch behind its upstream (`plan`): fast-forwarded at once when it is only behind, the Check out dialog when it has diverged; its actions, and the Git sequence run as one undo step (`run`), which the toast offers to take back with Cmd+Z.
- `operation.rs`: a merge, rebase, cherry-pick or revert stopped on conflicts (`Repo::in_progress`): Abort, Continue and Skip (`Repo::operation_step`). Each first copies the stopped operation (its files in the Git folder, the index with its conflicts, the files that differ from HEAD), so Undo puts it back whole, only when nothing changed since. `app/operation.rs` holds the banner over the history and the Abort confirmation.
- `push.rs`: the Push dialog's model (Push… ⇧⌘P and the palette): remote, remote branch, `--set-upstream` for a new one, Force push as `--force-with-lease` (asked twice when it replaces commits), Push all tags; its preview is `Preview::push`. The toolbar's Push stays direct (`Tab::push`).
- `menu.rs`: the macOS menu bar (Fourchette, File, Edit, View, Repository, Window, Help, laid out like the original's): a tested model of titles, shortcuts and greyed-out rules, and the AppKit menu. A pick is queued and turned into the `Cmd` of its button in `App::raw_input_hook` (Edit items go to the focused text field). `build.rs` links `assets/Info.plist` into the binary so the menu says "Fourchette" outside an .app bundle.
- `modal.rs`: the frame every dialog shares (#91): backdrop, centred card with shadow and 14 pt corners, header (icon tile, title, status line with branch pills, cross), body, footer (hint with keycaps, Cancel then the main action on the right, red when destructive), three widths, Enter and Esc, the scale-and-fade appearing. Also the choice cards, check rows, text fields and notes inside dialogs, and the card, backdrop and animation that Settings, the palette, the switcher and the toasts use. Call `ui::menu_rows` first in a context menu.
- `titlebar.rs`: no title bar strip, like the original. The content fills the window, the toolbar is its top edge (dragging its empty background moves the window, a double-click zooms it), and on macOS the red, yellow and green buttons are moved onto the toolbar's centre line, with `INSET` left free for them. `--screenshot` shows the egui content only, without those buttons.
- `library.rs`: the repository list (`~/Library/Application Support/Fourchette/repos.json`): groups, favourites, last opened, the chosen GitHub project.
- `app/view_tabs.rs`: the Changes · History · Project tabs above the history (they replaced the sidebar's first three entries): the active one is derived from the tab's state (`active_view`), Project is greyed with the reason when the repository has no GitHub project (`project_unavailable`). The board takes the centre under them; the sidebar and details panel stay.
- `app/issue.rs`: the issue of the current branch (the first number of its name that is an issue of the repository on the project): its chip in the toolbar, which opens its detail on the board, and the offer to move it, In progress after a checkout and In review once `gh pr list --head` finds its pull request. The move is the board's (`Tab::project_move`); Settings › Git can make it without asking, and its confirmation offers Undo, which moves the status back on the project (not Cmd+Z, which is Git's).
- `github.rs`: the GitHub project through `gh` in a background thread: the projects of the repository, then the project's items in the project's order through `gh api graphql` (types, labels, fields, sub-issues, linked pull requests), shared by the sidebar section and the board. A card dragged on the board or moved with "Move to" changes at once on screen; its Status and its position are then written (`updateProjectV2ItemFieldValue`, `updateProjectV2ItemPosition`) by one thread in order, and a refused write puts it back with the reason. Nothing else is ever written.
- `pulls.rs`: the pull requests of the repository and the CI of their last commit, read with one `gh api graphql` query in a background thread (every open one, plus the 30 merged or closed most recently updated; forks left out) on opening the repository, after a fetch or pull, and on the project's reload (`Tab::pulls_load`, `pulls_poll` in `app/project.rs`). Each branch shows its open pull request, else its latest, as a badge drawn by `ui::pr_badge` in its graph label and sidebar row (local branches and those of the remote read), with `#n title`, state and checks on hover. Without `gh` or outside GitHub nothing shows.
- `update.rs`: updates from the GitHub releases (pre-releases included), with `curl`: checked at launch then every 6 hours, only when the app runs from an .app bundle (not under `cargo run`). A release is newer when its version is higher, or the same version with its tag on another commit than the build's (`build.rs` embeds it as `FOURCHETTE_COMMIT`). The install checks the .dmg's SHA-256, copies the app next to the installed one, swaps them by renaming and relaunches; `app/updater.rs` holds the toolbar pill, the dialog and the polling.
- `ai.rs`: Claude Code through the person's subscription (`claude -p`, no tools, API keys hidden).
- `resolve.rs`: Claude's resolution of a conflicted file: it reads the file with its markers and the subjects of the commits on each side that touch it, and sends back the whole file. `app/resolve.rs` holds the window (a few files at a time in the background, applied one by one); Apply writes the file, Apply and mark resolved also stages it, and Undo puts the markers back and, with `Snapshot::resolved`, the conflict (`update-index --unresolve`).
- `changelog.rs`: the changelog Claude writes for one audience (developers, product, leadership, end users): one section per tag reachable from HEAD and "Unreleased" with the open pull requests, from the commit subjects and the titles `gh` gives. `app/changelog.rs` holds its window; CHANGELOG.md is written as one Undo step (`Snapshot::written`), and a release's notes are replaced on GitHub only after a confirmation.
- `pr_draft.rs`: the pull request Claude drafts for the current branch (Assistant menu, palette): the diff and commit subjects against `ai::base_branch`, and the issue whose number the branch name carries, read with `gh issue view`; a Conventional Commits title and an English body ending with `Closes #n`. `app/pr_draft.rs` holds its window: the text is editable, Copy copies it, and Create runs `gh pr create` only on that click, once the branch is on GitHub (it never pushes).
- `preview.rs`: the small animated graph that shows what an action does to a branch (a model of commits, lanes, labels and before/after scenes, and a painter that slides between them), its rows `ROW` apart; the Check out dialog draws it beside the list of actions (`modal::list_row`, same row height), with a Now / After switch, and the Pull, Merge and Push dialogs can use it.
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
