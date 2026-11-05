# Mechanics sheet — Fork (original) → Fourchette (rebuild)

Target: `/Applications/Fork.app/Contents/MacOS/Fork` (Mach-O arm64, Swift/AppKit), sha256 `a2b5ee90…bfca69`.
Ghidra's bridge returned no result for these functions, so they were read with the built-in
lifter and the raw disassembly. Evidence ids are in brackets.

## Commit graph drawing

| Value | Original | Where |
|---|---|---|
| Lane width | 11 pt per lane (`lane * 11`) | `sub_100102064` (`mov w23,#0xb`), `sub_100105180`, `sub_100105e34` [ev_e6386e8f, ev_c717e265] |
| Graph left inset | 3 pt (`x = lane*11 + 3`) | `sub_100102064` (`fmov d0,#3.0`), `sub_100105180` |
| Line width | 1.5 pt | `sub_100102064` (`fmov d0,#1.5` → `setLineWidth:`) |
| Curve offset for lane changes | 5 pt (bezier control points ±5) | `sub_100102064` (`fmov #5.0 / #-5.0`) |
| Lines meet at | row middle (`height * 0.5`) | `sub_100102064` (`fmov d1,#0.5`) |
| Commit dot | filled oval 5×5 centred on the lane | `sub_100105180` (`-2.5`, `5.0, 5.0`) |
| Highlighted commit (HEAD) dot | 12×12 circle, white fill, lane-colour 2 pt ring, chevron inside | `sub_100105180` (`-6.0`, `12.0, 12.0`, `#2.0`) |
| Colour choice | `palette[colourId % 9]` | `sub_100102064` (`sdiv/msub` by palette count) |
| Graph area width | `lanes * 11` added to the column | `sub_100105e34` (`fmov d2,#11.0; fmul`) |
| Ref label width | text width + 10 padding + 20 icon | `sub_100105e34` (`#10.0`, `#20.0`) |
| Ref label inset | 2 pt top/left, height − 4 | `sub_100105e34` (`#2.0`, `#-4.0`) |
| Gap between ref labels | 2 pt (compact) or 4 pt | `sub_100105e34` (`fcsel` between `#2.0`/`#4.0`) |

### Branch colour palette (9 colours, built once in `sub_1001e0440`, constants at `0x10060a080`)

| # | RGB | Hex |
|---|---|---|
| 0 | 1.000, 0.584, 0.008 | #FF9502 (orange) |
| 1 | 1.000, 0.800, 0.000 | #FFCC00 (yellow) |
| 2 | 1.000, 0.231, 0.188 | #FF3B30 (red) |
| 3 | 0.635, 0.518, 0.369 | #A2845E (brown) |
| 4 | 0.392, 0.855, 0.220 | #64DA38 (green) |
| 5 | 0.110, 0.678, 0.973 | #1CADF8 (blue) |
| 6 | 0.796, 0.451, 0.882 | #CB73E1 (purple) |
| 7 | 0.557, 0.557, 0.569 | #8E8E91 (grey) |
| 8 | 1.000, 0.161, 0.408 | #FF2968 (pink) |

## Git commands the original runs (strings + references)

| Purpose | Command / format | Where |
|---|---|---|
| Commit details | `--pretty=format:%H~#.%aN~#.%aE~#.%at~#.%cN~#.%cE~#.%ct~#.%P~#.%B` | `sub_100399eec` [ev_5cd6532d] |
| Refs | `for-each-ref --format=%(objectname)\t%(refname)\t%(HEAD)\t%(upstream)\t%(*objectname)…` | `sub_100296878` [ev_6b3d67c7] |
| Working copy | `--no-optional-locks status --porcelain --renames --untracked-files=all -z [--ignored] -- <paths>`; asks for just the changed paths when ≤ 1000 (`0x3e8`), otherwise refreshes everything | `sub_1003ab2d8` [ev_580e8e14] |
| Field separator | `~#.` | same strings |

Short option names (≤ 15 bytes) are packed into the code as numbers rather than stored as text.
They were decoded byte by byte from the `mov`/`movk` constants, e.g. `0x2d2d 0x7270 0x6e75 0x65` = `--prune`.

## Actions (decompiled, built-in lifter — Ghidra's bridge returned no result for this file)

| Action | What the original runs / does | Where |
|---|---|---|
| Fetch | `fetch --prune [--tags] [--force] <remote>` or `--all`, then `--progress`; env `NO_PROMPT`. Sheet title "Fetch '<remote>'". One remote by default ("fetchSheetFetchAllRemotes" = false). | `sub_1000c56b8` [ev_986f87a0], `sub_10045e7d4` [ev_59f82972] |
| Fetch result | "Already up to date" when no refs changed, otherwise "Fetched N branch"/"branches"; "Could not resolve host '…'" on network errors | `sub_1000c56b8` |
| Push | `-c push.default=upstream push <remote> <local>:refs/heads/<remote branch> [--force-with-lease] [--tags] [--set-upstream] --verbose --progress`; env `GIT_LFS_PROGRESS` | `sub_10016e1ec` [ev_6f10b85c] |
| Force push | `push <remote> <local>:refs/heads/<branch> --force-with-lease --verbose --progress` | `sub_10016dd54` [ev_3053161c] |
| Push result | "Everything is up-to-date" when git says up-to-date; "Could not resolve host '…'"; "Updated '…'" | `sub_10016e1ec` |
| Commit | Message written to a temp file (UTF-8), then `[-c gpg.program=…] commit [--gpg-sign=…] [--signoff] [--amend] [--no-verify] --file=<temp>`; falls back to `-m <message>` if the file can't be written | `sub_100318828` [ev_eaf0a00d] |
| Commit result | "Committed N file(s) to <branch> (<sha>)" / "Amended commit with N file(s) in <branch>"; "repository is locked" when `index.lock` exists | `sub_100318828` |
| Discard | Tracked files: reset through git with the paths on stdin. **Untracked files are moved to the Trash** (NSFileManager), not deleted | `sub_100053aac` [ev_86e3bc3d] |
| Locked repository | On `index.lock': File exists.` it retries up to **3** times, waiting **0.5 s × attempt** ("Retrying in %.1f") | `sub_100053aac` |
| Auto status refresh | Refreshes when more than "automaticStatusUpdateInterval" seconds passed (default **60**; 60.0 is also the fallback) | `sub_10048580c` [ev_28a44267] |
| Auto fetch | "fetchRemotesAutomatically" on by default; a timer (`updateTimerTick:`) fetches each open repository. In the rebuild it is a user setting (Settings window, ⌘,): on or off, every 5, 10, 30 or 60 minutes | `sub_10049189c` [ev_7c550d93], `sub_100490a98` |

## Settings and their defaults (`sub_10031b7cc`, registerDefaults) [ev_ce11436f]

| Setting | Default | Used in the rebuild |
|---|---|---|
| useTopologicalCommitOrder | true | history uses `--topo-order` |
| maxCommitCount | 100000 (`0x186a0`) | commit limit; a user setting (Settings window, "Diff and commit"), also set by the hidden palette command |
| fetchRemotesAutomatically | true | auto fetch on by default; a user setting (Settings window) |
| automaticStatusUpdateInterval | 60 (s) | status refresh every 60 s |
| commitSubjectLimitLow / High | 50 / 70 | subject counter turns orange above 50, red above 70, and Claude's prompt limits; user settings (Settings window, "Diff and commit"; low stays under high) |
| compactBranchLabels | true | 2 px gap between branch labels |
| createBranchSheetCheckout | true | "Switch to the new branch" ticked by default |
| saveStashSheetStageNewFiles | true | Stash includes new files (`stash push -u`) |
| deleteStashAfterApply | false | Apply keeps the stash; Pop removes it. A user setting (Settings › Git): on, Apply runs `stash pop` |
| pullSheetRebase / pullSheetStashAndReapply | false / false | Pull merges (`pull --no-edit --no-rebase <remote> <branch>`). User settings (Settings › Git): Merge or Rebase (`--rebase`), and "Stash and reapply" (`--autostash`) |
| fetchSheetFetchAllRemotes / fetchSheetFetchAllTags | false / false | Fetch one remote, no `--tags` (Option-click = all remotes) |
| pushSheetPushAllTags | false | no `--tags` |
| contextSize | 3 | diffs use `--unified=3`; a user setting (Settings window, "Diff and commit"), the open diff refreshes when it changes |
| localBranchSortOrder / remoteBranchSortOrder / tagSortOrder | 2 / 2 / 1 | meaning of the numbers not followed (*not used*) |
| repositoryManagerScanDepth | 6 | not used |

Not found in code (*estimated*): how often the automatic fetch runs (the timer interval was not
located; the two other timers in this area are 0.05 s and 0.2 s debounces, `sub_1003ade30`,
`sub_1004739bc`) — the rebuild uses 10 minutes by default, and the person can pick 5, 10, 30 or 60
in the Settings window. The exact git command Pull runs was not located.

## User settings (issue #16)

The Settings window (⌘, or "Settings…" in the palette, Esc to close) is laid out like macOS System
Settings (issue #52): a sidebar of sections with drawn icons (Appearance, Fetch, Git; Up and Down move
between them) and, on the right, the section's settings in rounded cards. Appearance holds the mode
(System, Dark, Light) and the variants as preview tiles; Fetch the automatic fetch (on/off and its
interval). Choices apply at once and are kept in repos.json, all in a `"settings"` object
(`"appearance"`, `"variant"`, `"auto_fetch"`, `"fetch_minutes"`). A file written before issue #52,
with `"appearance"` and `"variant"` at the top level, is still read (the `"settings"` place wins
when both hold a valid value) and is rewritten in the new layout at the next save. A missing or
invalid key falls back to its default (System, Classic, on, 10 minutes) without touching the
repository list.

The Git section (issue #18) holds Fork's pull sheet and stash defaults, all off by default:
Pull merges or rebases (`"pull_rebase"`), "Stash and reapply" puts uncommitted changes aside
during the pull (`"pull_autostash"`), and "Delete stash after Apply" (`"delete_stash_after_apply"`)
makes a stash's Apply a `stash pop`, which keeps the stash when applying it conflicts. Pull always
names its mode (`--rebase` or `--no-rebase`), so the setting wins over `pull.rebase` in the
person's Git config. A pull stopped on conflicts, or whose stashed changes conflict when put back
(Git then keeps them in the stash list as "autostash"), is reported as an error saying so.
Repository › Pull… (⇧⌘L, #101) opens Fork's pull sheet as a dialog: the remote branch to pull
(the upstream by default, else the branch of the same name), Merge or Rebase and "Stash and
reapply local changes" pre-filled from these settings, and the animated graph of the result. The
dialog's Merge adds `--ff`, so a branch with no commits of its own is always fast-forwarded.

### Merge (MergeWindowController, MergeTestGitCommand; issue #102)

Fork's Merge window has a "No fast-forward" box (`noFastForwardCheckBox`), and the original passes
`--no-ff` to `git merge` [ev_12829593]; its `Merge/MergeTestGitCommand.swift` checks a merge
before running it [ev_9be9b63c]. The rebuild opens one dialog from a branch's "Merge … into …",
the remote branch's "Merge … into current branch" and the palette:

| Choice | Git |
|---|---|
| Merge commit, a fast-forward possible | `merge --ff-only <branch>` |
| Merge commit with "No fast-forward", or diverged | `merge --no-ff --no-edit <branch>` (`--no-ff` named, so `merge.ff` in the person's config changes nothing) |
| Squash | `merge --squash <branch>`, then `commit -m "Squash <branch> into <head>"` when it staged anything |
| Conflict check, in a thread before anything runs | `git -c core.quotePath=false merge-tree --write-tree --name-only --no-messages HEAD <branch>`: exit 0 clean, 1 conflicts (tree id, then one file a line), 129 a Git older than 2.38 (the dialog says it cannot tell). It writes objects only: no ref, index or file moves |

The box shows only when a fast-forward is possible (the checked-out branch has no commits of its
own). One Undo takes the merge back; a merge stopped on conflicts is aborted (`merge --abort`), a
squash stopped on conflicts, which leaves no `MERGE_HEAD`, with `reset --merge`.

## Screens (from the compiled layouts in Resources/*.nib)

* Sidebar groups: **Branches, Remotes, Tags, Stashes** (`0x100610cc8…cf8`), a search field and a filter toggle (SidebarController.nib).
* Toolbar actions: **Fetch, Pull, Push, Stash, Branch** (strings in `__const`).
* Commit details (RevisionSummaryTabController.nib): subject, description, **SHA, AUTHOR, COMMITTER, PARENTS, REFS**, changed files with a diff under each.
* Commit view (CommitController.nib): unstaged / staged file lists, **Commit subject**, **Description**, **Commit** button, Amend.

## Menu bar (MainMenu.nib) [ev_1c8c0440aa55e0e10632401c67ff8c7fca7774108d82fe97f19b59ff97807be9]

Decoded from `Fork.app/Contents/Resources/Base.lproj/MainMenu.nib` (`investigation.cany/evidence/1c/`):
menus **Fork, File, Edit, View, Repository, Window, Help**, 137 items. Fourchette keeps that order and
those shortcuts where it has the action (`app/src/menu.rs`), and leaves out what it doesn't have
(New Window, Clone, Git Flow, Git LFS, Bisect, Apply Patch, Treemap, Statistics, Accounts, SSH keys…).

| Original item (action) | Shortcut | Fourchette |
|---|---|---|
| Preferences… (`preferences:`) | ⌘, | Settings… → `Cmd::Settings` |
| Hide / Hide Others / Show All / Quit | ⌘H, ⌥⌘H, —, ⌘Q | the same AppKit actions |
| File > New Tab (`newWindowForTab:`) | ⌘T | New Tab: the switcher, and the pick opens in a new tab (issue #59) |
| File > Close (`performClose:`) | ⌘W | Close Tab: the active tab, never the last |
| File > Open… (`openRepository:`) | ⌘O | Open Repository… (no key: ⌘O is Fourchette's switcher) |
| File > Open Recent | — | Open Recent, from the repository list |
| File > Quick Launch… (`showQuickOpenWindow:`) | ⌘P | Command Palette… ⌘P (⌘K also opens it) |
| Window > Quick Launch… | ⇧⌘O | File > Switch Repository… ⌘O |
| Edit > Undo / Redo / Cut / Copy / Paste / Select All | ⌘Z ⇧⌘Z ⌘X ⌘C ⌘V ⌘A | sent to the focused text field; Undo is `Cmd::Undo` otherwise |
| View > Show Commit Details (⌥⌘1) | ⌥⌘1 | Show Details Panel ⌘J (Fourchette's key), Details on the Right / at the Bottom |
| View > Show HEAD (`showHeadRevision:`) | ⌘0 | the same |
| View > Refresh (`refresh:`) | ⌘R | the same |
| View > Enter Full Screen | ⌃⌘F | the same (macOS also adds its own 🌐F item and hides ours) |
| Repository > Commit (`showCommitView:`) | ⇧⌘C | commits, like the Commit button |
| Fetch… / Quick Fetch | ⇧⌘F / ⌥⇧⌘F | Fetch / Fetch All Remotes |
| Pull… (`showPullSheet:`) | ⇧⌘L | the Pull dialog (#101): remote branch, Merge or Rebase, Stash and reapply; the toolbar's Pull still pulls at once |
| Push… | ⇧⌘P | Push (no sheet in Fourchette) |
| Save Stash… (`saveStash:`) | ⇧⌘H | Stash Changes; Pop Stash added (toolbar's Pop) |
| Checkout Branch… (`showQuickLaunchCheckout:`) | ⌘B | the branch palette |
| New Branch… / New Tag… | ⇧⌘B / ⇧⌘T | the new branch / tag dialog at HEAD |
| Show in Finder / Open in Terminal | ⌃⌘O / ⌃⌘T | the same |
| Window > Minimize / Zoom / Bring All to Front | ⌘M | the same AppKit actions |
| Window > Show Previous / Next Tab (added by macOS for its tabs) | ⌃⇧Tab / ⌃Tab | the same names, with Safari's ⇧⌘[ / ⇧⌘]; ⌃Tab and ⌃⇧Tab work too |
| Help > Issue Tracker, Website | — | Fourchette on GitHub, Report an Issue |

## Window frame (no title bar, issue #53)

Like the original's main window, there is no title bar strip: the content view fills the window
(`fullsize_content_view`, transparent title bar, title hidden; the title "Fourchette" stays for Mission
Control and the Window menu). The 60 pt toolbar is the top edge. The close, minimise and zoom buttons
are moved onto its centre line (close at x = 20 pt, y centred in 60 pt, AppKit's own spacing), and the
toolbar keeps 86 pt free on its left for them (none in full screen, where macOS hides them).
Dragging the toolbar's empty background moves the window; a double-click zooms it.

## New features (not in the original — the person asked for a real edge)

* **Universal Undo** (toolbar button, ⌘Z, palette). Before every local action (commit, amend,
  checkout, merge, rebase, reset, cherry-pick, revert, branch/tag create or delete, stash, pop,
  discard, pull) the app records HEAD, every branch and tag tip and the stash list. Undo puts
  them back: commits are undone with `reset --soft` so the changes return staged; other moves use
  `reset --keep` (refuses rather than lose local edits); branches/tags are restored with
  `update-ref`; a new stash is popped back; a dropped stash is stored again; discarded edits are
  saved with `git stash create` first and re-applied; a new file sent to the Trash is moved back.
  A rebase or merge left stopped on conflicts (a pull's) is aborted first. A pull also saves the
  uncommitted changes: when it moved them onto the pulled commits ("Stash and reapply") and
  `reset --keep` refuses, Undo resets with `--hard` and re-applies them as they were, only if they
  were not edited since the pull (otherwise it refuses), and drops Git's "autostash" copy. A
  dropped stash comes back under its own message.
  30 steps deep. Actions that change nothing are not recorded. Push and fetch are not undoable
  (they touch the server).
* **Command palette** (⌘K or ⌘P): type to run any action, check out or merge any branch, open a
  recent repository, go to a tag, or jump to a commit by message, author or hash. It is modelled
  on the original's **Quick Launch** window (decompiled, see below).

* **AI assistant** (toolbar "Assistant", ⌘K "Assistant" group). Not in the original.
  Uses the person's **Claude subscription** through the Claude Code command line, never an API key:
  `claude -p --output-format json --tools "" --no-session-persistence --strict-mcp-config
  --setting-sources project [--json-schema …]`, prompt on stdin, run from the temp folder, with
  `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_USE_BEDROCK/VERTEX` removed from its
  environment. `--bare` is **not** used because it only accepts an API key. `--tools ""` means
  Claude can't run anything: it only reads what the app sends.
  * **Branch clean-up**: for each local branch (not the current one or the base) the app sends:
    merged into base (`branch --merged`), commits not in base / behind (`rev-list --left-right
    --count`), upstream and whether it's gone (`%(upstream:track)`), last commit date, author,
    subject. Base = `origin/HEAD`'s branch, else main/master/develop/trunk. Claude answers
    delete / keep / review + reason (JSON schema). The app keeps only branches that exist, turns
    "delete" into "review" for a branch with unmerged commits, and pre-ticks only "delete". The
    person ticks and confirms; merged branches go with `branch -d`, others with `branch -D`;
    one Undo step restores them all.
  * **Suggest a commit message** from `diff --cached` (+ 12 recent subjects for style; subject
    asked ≤ 50 characters, the original's commitSubjectLimitLow).
  * **Explain a commit** in plain words from `show --stat --patch`.
  * At most 60 000 characters are sent per question.

### The original's own Claude / OpenAI features (decompiled, built-in lifter)

The original already has "Generate commit message with Claude" / "…with OpenAI" (menu
`sub_1003ba93c`) and "Code Review with OpenAI…" (`AiCodeReviewController`, `OpenAiService`).

| Rule | Value | Where |
|---|---|---|
| Where Claude Code is looked for | exactly `/opt/homebrew/bin/claude`, then `~/.local/bin/claude` (`~` expanded), first that exists | `sub_1003e513c` [ev_8419e4f0], strings 0x1006476b0 / 0x1006476d0 |
| Not found | "Cannot find Claude" | `sub_1003e4338` [ev_b4685c69] |
| How it's run | `claude "<prompt>"` — one argument, working folder = the repository, output not a terminal (so Claude Code answers once and exits); Claude reads the repository itself | `sub_1003e4338` → `sub_1002560f8` [ev_bf0c8f66]; array header 0x100604560 = 1 element |
| Prompt | "Generate commit message for {staged changes \| staged changes combined with the last commit (i.e. `git diff --cached HEAD^`)}.\n- Only respond with the commit message.\n- Start directly with the subject line (no preamble).\n- The header must be less than {commitSubjectLimitLow} (soft limit) and {commitSubjectLimitHigh} (hard limit).\n- Hard wrap lines at {pageGuideLinePosition} characters.\n- Don't use the 'generated' footer." | `sub_1003e4338` |
| Reading the answer | split at the first line break: subject before, description after, both trimmed of newlines | `sub_1003e4338` (`range(of: "\n")`, `trimmingCharacters(in: .newlines)`) |
| Progress text | "Generating with Claude…" | `sub_1003e4338` |
| OpenAI variant | same rules + "Do not start any lines with the hash symbol." and "Below is my git diff:\n\n```\n<diff>" | `sub_1004a7d4c` |
| OpenAI code review prompt | "Make code review for changes.\nDo not thank.\n\nBelow is the git diff:" | `sub_1004a85ec` |

Rebuild: same two Claude locations first (then the PATH), same "Cannot find Claude" text, the
original's prompt word for word with 50 / 70 and the amend wording + `git diff --cached HEAD^`,
same first-line split, same progress text. Differences, on purpose: Claude gets **no tools** and
runs from the temp folder (so it can't touch the repository); the diff is therefore sent after the
prompt the way the original's OpenAI prompt does. `pageGuideLinePosition`'s default wasn't found:
72 is used (*estimated*). Branch clean-up and commit explanation have no counterpart in the original.

### Worktrees (decompiled, built-in lifter)

Git refuses to delete a branch that is checked out in a worktree, and Claude Code creates many.

| Rule in the original | Value | Where |
|---|---|---|
| Delete-branch sheet | title "Delete Branch" / "Delete N Branches", text "Delete local branch from your repository" / "Delete local branches from your repository" | `sub_100070d94` [ev_1cfd7358] |
| Worktree in that sheet | only when **one** branch is deleted and it is in `worktreesByFullReference`: a warning icon, the worktree's folder name (`lastPathComponent`) and a checkbox "Also delete worktree"; hidden for several branches | `sub_100070d94` |
| Remote option | "Also delete remote branch" / "…corresponding remote branch(es)" | `sub_100070d94` |
| Delete-worktree sheet | title "Delete worktree '<folder>'", text "Do you want to delete worktree '<folder>'?", button "Delete" | `sub_1002008ec` [ev_49d9e7d2] |
| Sidebar | Worktrees group (main worktree + linked ones), context menu: Open, Copy Worktree Path, Remove; "showWorktrees" default false | class names 0x10060a4b0…0x100612fb0, `sub_10031b7cc` |

Rebuild:
* Worktrees are read with `git worktree list --porcelain` (main = first entry; `prunable` or a
  missing folder = "folder gone"; `locked` noted).
* Deleting a branch (sidebar menu, ⌘K, or the AI clean-up) removes its worktree first, then
  `branch -d` (merged) / `-D` (ticked "Delete even though it isn't merged"). The sidebar sheet
  uses the original's wording and "Also delete worktree" (ticked by default — *estimated*, the
  nib's default wasn't read). Unticked → Delete is disabled ("Git can't delete a branch that is
  checked out in a worktree").
* Uncommitted work in the worktree is saved first (`git add -A` + `git stash create` inside it),
  then `git worktree remove --force` (twice if locked); a missing folder is cleaned with
  `git worktree prune`. *The original's exact remove command wasn't located (short strings are
  inline); this is estimated.*
* Undo re-creates the worktree with `git worktree add <path> <branch>` and re-applies the saved work.
* AI clean-up: Claude is told which branches sit in a worktree and whether it has uncommitted
  changes; a dirty worktree is never pre-ticked.
* The Worktrees sidebar section is shown whenever linked worktrees exist (the original hides it by
  default — changed because the person uses Claude Code worktrees a lot). "Clean up missing
  worktrees" runs `git worktree prune`.

### The original's Quick Launch (decompiled, built-in lifter)

| Rule | Value | Where |
|---|---|---|
| Item row height | 25 pt | `tableView:heightOfRow:` 0x10011464c → `sub_100116548` (`fmov d8,#25.0`) [ev_90393ac3] |
| Group header row height | 17 pt (rows of `HeaderCommandProviderItem`) | `sub_100116548` (`fmov d8,#17.0`) |
| Groups (providers) | DefaultCommandProvider (commands), ReferenceCommandProvider (branches/tags), WorkspaceCommandProvider (repositories), RepositoryFileCommandProvider (files) | class names `0x100607b80…0x100610e60` [ev_fdc1f5ed] |
| Row content | icon, title, secondary text | QuickLaunchTableCellView outlets `iconImageView`, `titleTextField`, `secondaryTextField` |
| Placement | x = main window midX − width × 0.5; y = main window midY − height / 3 (AppKit, bottom-left origin) → centred, a third of its height above the middle | `sub_1001f8330` (`fmov #-0.5`, `fmov #-3.0; fdiv`) [ev_c141765a] |
| Checkout mode | `showQuickLaunchCheckout:` opens it listing branches; the repository status view's branch button calls it | 0x1001f37c0 → `sub_1001f8330(1)`; 0x1002f00e8 [ev_4d0a888b] |
| Hidden command | `settings maxCommitCount <n>`: exactly 3 words split on spaces, key must be `maxCommitCount`, the number may start with + or − | `handleSettingsCommand(_:)` `sub_100114e44` [ev_d243fa8b] |
| Recent repositories | "Open Recent Repository:" header | QuickLaunchWindowController.nib |

Rebuild: rows 25 px, headers 17 px, groups Commands / Branches / Tags / Repositories / Commits,
same placement formula, branch button opens the branch-only mode, `settings maxCommitCount <n>`
works (only positive numbers are taken). The file provider isn't rebuilt (there is no file view).

### The original's confirmation alerts (decompiled)

All follow one pattern: a question "Do you want to …?", the informative line "You can't undo
this action", then the action's verb and Cancel. Examples: "Do you want to discard the changes in
the selected lines?" + "Discard N Lines"/"Discard Line" (`sub_1003b95c0` [ev_2b8656df]),
"Do you want to delete the workspace '…'?" + "Delete" (`sub_10026d55c` [ev_ddde9283]),
"Do you want to remove the selected custom command?" + "Remove" (`sub_1000de8c0` [ev_72d353fe]).
The original has an `UndoManager` class only for text fields and the interactive rebase
(`undoRedoManager`, `undoButton`); there is no repository-wide undo — that is the new feature.
Rebuild: same question / verb / Cancel layout; the informative line says the action can be taken
back with Undo (⌘Z), because here it can.

## The new look (person's request: a modern, Linear-like style, issue #7)

The person first asked for a Kraken-like style, then for a calm, modern look in the spirit of
Linear. These are design choices, not values from the original:

* Lane width 16 px (original 11), line width 2 px (original 1.5), commit node = 5 px radius dot
  in the lane colour (original 5 pt dot), merge commits = 3.5 px hollow ring, a ring around the
  checked-out commit. The author's initials moved from the graph to the Author column.
* Row height 28 px — the original's row height was not found in code (*estimated*). Rows touch,
  so lines run on from one row to the next.
* Colours: the original 9-colour palette is kept for branches; the window uses near-black
  surfaces, subtle borders and one indigo accent. Text uses a four-size scale (11, 12, 13, 15).
* Appearance (issue #11): System (follows macOS, the default), Dark or Light, chosen in the
  Settings window, the palette or with the status bar button and kept in repos.json (`"settings"."appearance"`). The light mode
  uses white surfaces and deepens the branch colours by 20% so they stay readable.
* Variants (issue #14, a design choice, not a measured value): Classic (the look above, silver
  fork), Gold (night-brown surfaces, amber accent, golden fork), Neon (night-violet surfaces,
  magenta accent, glowing pink and cyan fork) and Forest (deep green surfaces, green accent, copper
  fork). Each has a dark and a light palette, so Appearance works with all of them; the branch
  colours are the same in all. Chosen in the Settings window or the palette ("Variant: …"), kept in
  repos.json (`"settings"."variant"`, Classic by default), and the Dock icon changes at once.
* Colour id: a new colour is given each time a new branch line starts (*estimated*: the original
  stores a colour byte per lane; how it picks it was not fully followed).
