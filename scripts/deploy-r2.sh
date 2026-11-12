#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

R2_SLUG="${R2_SLUG:-md-tui}"
DIST="${DIST:-web/dist}"
export CLOUDFLARE_ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-2e169e038d9f430944291e98aa79e684}"
: "${CLOUDFLARE_API_TOKEN:?Set CLOUDFLARE_API_TOKEN before deploying}"
export CLOUDFLARE_API_TOKEN

if [ "${SKIP_INSTALL:-}" != 1 ]; then
  pnpm install --frozen-lockfile
fi
if [ "${SKIP_BUILD:-}" != 1 ]; then
  pnpm build
fi

if [ ! -f "$DIST/index.html" ]; then
  echo "error: $DIST/index.html missing; build the app first" >&2
  exit 1
fi

wrangler_cmd() {
  if [ -x node_modules/.bin/wrangler ]; then
    node_modules/.bin/wrangler "$@"
  elif command -v wrangler >/dev/null 2>&1; then
    command wrangler "$@"
  else
    npx wrangler "$@"
  fi
}

content_type() {
  local base
  base="$(basename "$1")"
  case "$base" in
    SHA256SUMS|VERSION) printf '%s' 'text/plain; charset=utf-8' ;;
    *.html) printf '%s' 'text/html; charset=utf-8' ;;
    *.js|*.mjs) printf '%s' 'application/javascript; charset=utf-8' ;;
    *.css) printf '%s' 'text/css; charset=utf-8' ;;
    *.svg) printf '%s' 'image/svg+xml' ;;
    *.png) printf '%s' 'image/png' ;;
    *.webp) printf '%s' 'image/webp' ;;
    *.ico) printf '%s' 'image/x-icon' ;;
    *.json) printf '%s' 'application/json; charset=utf-8' ;;
    *.txt|*.sh) printf '%s' 'text/plain; charset=utf-8' ;;
    *.webmanifest) printf '%s' 'application/manifest+json' ;;
    *.woff2) printf '%s' 'font/woff2' ;;
    *.woff) printf '%s' 'font/woff' ;;
    *.map) printf '%s' 'application/json; charset=utf-8' ;;
    *) printf '%s' 'application/octet-stream' ;;
  esac
}

while IFS= read -r -d '' file; do
  path="${file#"$DIST"/}"
  mime="$(content_type "$path")"
  wrangler_cmd r2 object put "techdemos-static/$R2_SLUG/$path" --file "$file" --content-type "$mime" --remote
done < <(find "$DIST" -type f -print0)

# R2 objects have no automatic directory-index behavior.
wrangler_cmd r2 object put "techdemos-static/$R2_SLUG" --file "$DIST/index.html" --content-type 'text/html; charset=utf-8' --remote
wrangler_cmd r2 object put "techdemos-static/$R2_SLUG/" --file "$DIST/index.html" --content-type 'text/html; charset=utf-8' --remote
printf 'Public probe URL: https://pub-4501701e4d0343beafbbe85c4411530e.r2.dev/%s/\n' "$R2_SLUG"
