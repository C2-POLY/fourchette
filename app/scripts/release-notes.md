# Fourchette

A Git client for macOS: history graph, branches, worktrees, stashes, undo for every local action, a Claude assistant and the GitHub project of the open repository.

## Install

1. Open `Fourchette-<version>.dmg`.
2. Drag Fourchette to Applications.

## First open

This build is not notarized, so macOS blocks it the first time. Either:

- open Fourchette, then go to System Settings > Privacy & Security and click "Open Anyway"; or
- run `xattr -dr com.apple.quarantine /Applications/Fourchette.app` in a terminal.

## Requirements

- `git` (Xcode Command Line Tools).
- `gh` (optional, for the GitHub project).
- `claude` (optional, for the assistant).
- Apple Silicon or Intel, macOS 11 or later.
