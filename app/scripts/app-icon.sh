#!/usr/bin/env bash
# Renders assets/icon.svg into the icons made from it, with Chrome headless and sips:
# assets/icon.png (1024, the window and Dock icon), assets/icon.icns (the .app bundle)
# and the site's favicon.png (64) and apple-touch-icon.png (180).
# Run it after changing the SVG, then commit what it wrote: release.sh uses them as they are.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ASSETS="$(cd "$SCRIPT_DIR/../assets" && pwd)"
SITE="$(cd "$SCRIPT_DIR/../../site/public" && pwd)"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"

if [ ! -x "$CHROME" ]; then
  echo "error: Chrome not found; set CHROME to its binary" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

"$CHROME" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
  --window-size=1024,1024 --screenshot="$ASSETS/icon.png" "file://$ASSETS/icon.svg" 2>/dev/null

# Every size the .icns holds, scaled down from the 1024 render.
SET="$TMP/icon.iconset"
mkdir "$SET"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$ASSETS/icon.png" --out "$SET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$ASSETS/icon.png" --out "$SET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$ASSETS/icon.icns"

sips -z 64 64 "$ASSETS/icon.png" --out "$SITE/favicon.png" >/dev/null
sips -z 180 180 "$ASSETS/icon.png" --out "$SITE/apple-touch-icon.png" >/dev/null
echo "Rendered icon.png, icon.icns, favicon.png and apple-touch-icon.png"
