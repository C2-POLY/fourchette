#!/usr/bin/env bash
# Builds a universal Fourchette.app and a .dmg, ad hoc signed.
# Without --publish, it only builds into app/target/release-bundle/.
# With --publish, it creates or updates the GitHub pre-release v<version>.
# With --notes, it builds nothing and only rewrites the notes of the existing release v<version>.
# The notes come from release-notes.sh (preview them with `scripts/release-notes.sh`).
set -euo pipefail

# Resolve paths from this script's folder, so it runs from any cwd.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="C2-POLY/fourchette"
DMGBUILD_VERSION="1.6.7"
PUBLISH=0
NOTES_ONLY=0

case "${1:-}" in
  "") ;;
  --publish) PUBLISH=1 ;;
  --notes) NOTES_ONLY=1 ;;
  *) echo "usage: $0 [--publish | --notes]" >&2; exit 1 ;;
esac

cd "$APP_DIR"

# Version from Cargo.toml.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
if [ -z "$VERSION" ]; then
  echo "error: no version in app/Cargo.toml" >&2
  exit 1
fi
echo "Fourchette $VERSION"
TAG="v$VERSION"

# --notes: the release's notes from its tag's history, nothing built or uploaded.
if [ "$NOTES_ONLY" -eq 1 ]; then
  git -C "$APP_DIR" fetch origin --tags --force --quiet
  gh release view "$TAG" --repo "$REPO" >/dev/null || { echo "error: no release $TAG on GitHub" >&2; exit 1; }
  NOTES="$(mktemp -t fourchette-notes)"
  trap 'rm -f "$NOTES"' EXIT
  "$SCRIPT_DIR/release-notes.sh" "$VERSION" "$TAG^{commit}" > "$NOTES"
  gh release edit "$TAG" --repo "$REPO" --notes-file "$NOTES"
  echo "Updated the notes of release $TAG"
  exit 0
fi

# Refuse to publish from a dirty tree or from a commit that is not origin/main.
if [ "$PUBLISH" -eq 1 ]; then
  if [ -n "$(git -C "$APP_DIR" status --porcelain)" ]; then
    echo "error: the working tree is not clean; commit or stash first" >&2
    exit 1
  fi
  git -C "$APP_DIR" fetch origin main --quiet
  if [ "$(git -C "$APP_DIR" rev-parse HEAD)" != "$(git -C "$APP_DIR" rev-parse origin/main)" ]; then
    echo "error: HEAD is not origin/main; merge first, then publish from main" >&2
    exit 1
  fi
  git -C "$APP_DIR" fetch origin --tags --force --quiet
  if [ "$(git -C "$APP_DIR" rev-parse "$TAG^{commit}" 2>/dev/null)" != "$(git -C "$APP_DIR" rev-parse HEAD)" ]; then
    echo "error: tag v$VERSION is not on main; move it first (see Versioning in CLAUDE.md)" >&2
    exit 1
  fi
fi

# Build both architectures and join them into one binary.
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

OUT="$APP_DIR/target/release-bundle"
BUNDLE="$OUT/Fourchette.app"
DMG="$OUT/Fourchette-$VERSION.dmg"

rm -rf "$OUT"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"

lipo -create \
  "$APP_DIR/target/aarch64-apple-darwin/release/fourchette" \
  "$APP_DIR/target/x86_64-apple-darwin/release/fourchette" \
  -output "$BUNDLE/Contents/MacOS/fourchette"

# Bundle resources and the version keys.
cp "$APP_DIR/assets/icon.icns" "$BUNDLE/Contents/Resources/icon.icns"
cp "$APP_DIR/assets/Info.plist" "$BUNDLE/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$BUNDLE/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$VERSION" "$BUNDLE/Contents/Info.plist"

# Ad hoc signature: no Developer ID, so no notarization.
codesign --force --sign - --timestamp=none "$BUNDLE"
codesign --verify --strict "$BUNDLE"

# The disk image: the app and a link to /Applications, laid out on the background
# of assets/dmg (see dmg-settings.py). dmgbuild lives in its own venv under target/.
VENV="$APP_DIR/target/dmgbuild"
if ! "$VENV/bin/python" -c "import importlib.metadata as m, sys; sys.exit(m.version('dmgbuild') != '$DMGBUILD_VERSION')" 2>/dev/null; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install --quiet --disable-pip-version-check "dmgbuild==$DMGBUILD_VERSION"
fi
"$VENV/bin/dmgbuild" -s "$SCRIPT_DIR/dmg-settings.py" \
  -D app="$BUNDLE" -D assets="$APP_DIR/assets" \
  "Fourchette $VERSION" "$DMG" >/dev/null 2> >(grep -v 'is deprecated' >&2)

echo "Built:"
echo "  $BUNDLE"
echo "  $DMG"

if [ "$PUBLISH" -eq 0 ]; then
  exit 0
fi

# Publish or update the pre-release, with notes generated from the commits since the last version.
NOTES="$OUT/release-notes.md"
"$SCRIPT_DIR/release-notes.sh" "$VERSION" HEAD > "$NOTES"
if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  gh release upload "$TAG" "$DMG" --repo "$REPO" --clobber
  gh release edit "$TAG" --repo "$REPO" --notes-file "$NOTES"
  echo "Updated release $TAG"
else
  gh release create "$TAG" "$DMG" --repo "$REPO" --prerelease --verify-tag \
    --title "Fourchette $VERSION" --notes-file "$NOTES"
  echo "Created release $TAG"
fi
