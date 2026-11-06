#!/usr/bin/env bash
# Prints the notes of a release to stdout: the intro line of release-install.md, a "What's new"
# section made from the commit subjects since the previous version, then the rest of
# release-install.md (install, first open, requirements).
#
#   scripts/release-notes.sh [<version> [<rev>]]
#
# <version> defaults to app/Cargo.toml's, <rev> to HEAD. The previous version is the highest
# v* tag below <version> (none: the whole history). Run it in the repository whose history to
# read. Only Conventional Commits count: feat goes under New, fix under Fixed, perf under
# Improved; other types and the `site` scope are left out. The app reads this section back
# (src/update.rs, `parse_news`), so keep the headings and the trailing "(#n)" as they are.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$SCRIPT_DIR/../Cargo.toml" | head -n 1)}"
REV="${2:-HEAD}"
INSTALL="$SCRIPT_DIR/release-install.md"

# The highest vX.Y.Z tag below VERSION.
PREV="$(git tag -l 'v*' | awk -v want="$VERSION" '
  function key(v,   p) { sub(/^v/, "", v); if (split(v, p, ".") != 3) return -1; return p[1] * 1000000 + p[2] * 1000 + p[3] }
  /^v[0-9]+\.[0-9]+\.[0-9]+$/ { k = key($0); if (k < key(want) && k > best) { best = k; tag = $0 } }
  END { print tag }')"
RANGE="$REV"
if [ -n "$PREV" ]; then
  RANGE="$PREV..$REV"
fi

# The intro: release-install.md up to its first blank line.
awk 'NF == 0 { exit } { print }' "$INSTALL"
echo
git log --format=%s "$RANGE" | awk '
  match($0, /^[a-z]+(\([^)]*\))?!?: /) {
    head = substr($0, 1, RLENGTH); text = substr($0, RLENGTH + 1)
    type = head; sub(/[(!:].*$/, "", type)
    scope = ""
    if (match(head, /\([^)]*\)/)) scope = substr(head, RSTART + 1, RLENGTH - 2)
    if (scope == "site" || text == "") next
    # A comparison with another app (", like Xyz") stays out of the notes.
    sub(/, like [A-Z][A-Za-z]* \(#/, " (#", text)
    text = toupper(substr(text, 1, 1)) substr(text, 2)
    if (type == "feat") nw[++n] = text
    else if (type == "fix") fx[++f] = text
    else if (type == "perf") im[++p] = text
  }
  END {
    print "## What'"'"'s new"
    if (n + f + p == 0) { print ""; print "Small fixes and internal changes." }
    if (n) { print ""; print "### New"; print ""; for (i = 1; i <= n; i++) print "- " nw[i] }
    if (f) { print ""; print "### Fixed"; print ""; for (i = 1; i <= f; i++) print "- " fx[i] }
    if (p) { print ""; print "### Improved"; print ""; for (i = 1; i <= p; i++) print "- " im[i] }
  }'
echo
# The rest of release-install.md, after the intro and its blank line.
awk 'started { print; next } NF == 0 { started = 1 }' "$INSTALL"
