# Fourchette

A fast, native Git client for macOS, written in Rust: a history you can read as a graph, an undo for every local action, worktrees, your GitHub project as a board, and Claude to write commit messages and tidy branches.

**Website:** <https://c2-poly.github.io/fourchette/> · **Roadmap:** <https://github.com/orgs/C2-POLY/projects/1>

## Download

Get the latest `Fourchette-<version>.dmg` from [GitHub Releases](https://github.com/C2-POLY/fourchette/releases), open it and drag Fourchette to Applications. The build is not notarized yet, so macOS blocks the first open: click "Open Anyway" in System Settings > Privacy & Security. Fourchette then checks for new versions and installs them in one click.

It runs on macOS 11 or later, on Apple Silicon or Intel, and needs Git. The GitHub CLI (`gh`) is optional, for the project board, and so is Claude Code, for the assistant.

## Build from source

You need macOS, Rust 1.95 or later and Git. The GitHub CLI (`gh`) is optional, for the project board, and so is Claude Code, for the assistant.

```bash
git clone https://github.com/C2-POLY/fourchette.git
cd fourchette/app
cargo run --release -- ~/path/to/a/repository
```

Without an argument, it opens the current folder if it is a repository, else the tabs you left open, else a home page that lists your repositories by GitHub organisation.

## Contributing

Issues and pull requests are welcome. `CLAUDE.md` describes the architecture, the commands and how work items are tracked.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
