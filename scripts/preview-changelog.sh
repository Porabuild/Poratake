#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

BOLD=$'\033[1m'
DIM=$'\033[2m'
RED=$'\033[31m'
GREEN=$'\033[32m'
YELLOW=$'\033[33m'
CYAN=$'\033[36m'
MAGENTA=$'\033[35m'
RESET=$'\033[0m'

usage() {
  cat <<EOF
Preview the release notes that would be generated for the next Poratake release.

Uses the same commit-parsing logic as scripts/release.sh (feat: -> Features,
fix: -> Bug Fixes, other prefixes -> Internal), so the output matches what the
real release would publish.

Usage:
  scripts/preview-changelog.sh [version] [--internal] [--raw]

Arguments:
  version       Optional. If the version's tag exists (e.g. 1.13.0), shows the
                changelog of that release (commits between the previous tag and
                that tag). If it does not exist (a future version), previews the
                next release (commits between the latest tag and HEAD). Defaults
                to a patch-bump of the latest v* tag (e.g. v1.12.1 -> 1.12.2).

Options:
  --internal    Include the Internal section (matches internal_release_notes.txt).
                Default output matches the public release_notes.txt.
  --raw         Skip ANSI styling and print the markdown body as-is.
  -h, --help    Show this help.
EOF
}

VERSION=""
SHOW_INTERNAL=0
RAW=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --internal) SHOW_INTERNAL=1; shift ;;
    --raw)      RAW=1; shift ;;
    -h|--help)  usage; exit 0 ;;
    -*)
      printf "%sError:%s unknown option '%s'\n" "$RED" "$RESET" "$1" >&2
      exit 1
      ;;
    *)
      if [[ -z "$VERSION" ]]; then
        VERSION="$1"
      else
        printf "%sError:%s unexpected argument '%s'\n" "$RED" "$RESET" "$1" >&2
        exit 1
      fi
      shift
      ;;
  esac
done

LATEST_TAG="$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || echo "")"

if [[ -z "$VERSION" ]]; then
  if [[ -n "$LATEST_TAG" ]]; then
    BASE="${LATEST_TAG#v}"
    if [[ "$BASE" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
      VERSION="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$((BASH_REMATCH[3] + 1))"
    else
      VERSION="$(jq -r .version package.json)"
    fi
  else
    VERSION="$(jq -r .version package.json)"
  fi
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf "%sError:%s version must be X.Y.Z (got '%s')\n" "$RED" "$RESET" "$VERSION" >&2
  exit 1
fi

TAG="v${VERSION}"
HEAD_SHA="$(git rev-parse --short HEAD)"
HEAD_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  IS_RELEASED=1
  TARGET_REF="$TAG"
  PREVIOUS_TAG="$(git describe --tags --abbrev=0 --match 'v*' "${TAG}^" 2>/dev/null || echo "")"
else
  IS_RELEASED=0
  TARGET_REF="HEAD"
  PREVIOUS_TAG="$LATEST_TAG"
fi

if [[ -n "$PREVIOUS_TAG" ]]; then
  COMMITS="$(git log "${PREVIOUS_TAG}..${TARGET_REF}" --pretty=format:"%s" --no-merges)"
else
  COMMITS="$(git log "$TARGET_REF" --pretty=format:"%s" --no-merges)"
fi

FEATURES=""
FIXES=""
INTERNAL=""

while IFS= read -r commit; do
  [[ -z "$commit" ]] && continue

  if echo "$commit" | grep -qiE "^feat(\(.*\))?:"; then
    MSG="$(echo "$commit" | sed -E 's/^feat(\([^)]*\))?:[[:space:]]*//')"
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    FEATURES="${FEATURES}- ${MSG}"$'\n'
  elif echo "$commit" | grep -qiE "^fix(\(.*\))?:"; then
    MSG="$(echo "$commit" | sed -E 's/^fix(\([^)]*\))?:[[:space:]]*//')"
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    FIXES="${FIXES}- ${MSG}"$'\n'
  else
    MSG="$(echo "$commit" | sed -E 's/^(chore|refactor|docs|internal|tech-debt|style|test|ci|build|perf)(\([^)]*\))?:[[:space:]]*//')"
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    INTERNAL="${INTERNAL}- ${MSG}"$'\n'
  fi
done <<< "$COMMITS"

NOTES=""
if [[ -n "$FEATURES" ]]; then
  NOTES="${NOTES}## Features"$'\n'"${FEATURES}"
fi
if [[ -n "$FIXES" ]]; then
  [[ -n "$NOTES" ]] && NOTES="${NOTES}"$'\n'
  NOTES="${NOTES}## Bug Fixes"$'\n'"${FIXES}"
fi
if [[ "$SHOW_INTERNAL" -eq 1 && -n "$INTERNAL" ]]; then
  [[ -n "$NOTES" ]] && NOTES="${NOTES}"$'\n'
  NOTES="${NOTES}## Internal"$'\n'"${INTERNAL}"
fi
if [[ -z "$NOTES" ]]; then
  NOTES="No changelog provided"$'\n'
fi

hr() {
  printf "%s%s" "$BOLD" "$1"
  printf '━%.0s' {1..68}
  printf "%s\n" "$RESET"
}

print_header() {
  local color="$GREEN"
  printf "\n"
  hr "$color"
  printf "  %s%sPoratake Release Preview%s\n" "$BOLD" "$color" "$RESET"
  hr "$color"
  printf "  %sVersion:%s  %s\n" "$BOLD" "$RESET" "$VERSION"
  printf "  %sTag:%s      %s\n" "$BOLD" "$RESET" "$TAG"
  if [[ "$IS_RELEASED" -eq 1 ]]; then
    printf "  %sTarget:%s   %s %s(released)%s\n" "$BOLD" "$RESET" "$TAG" "$DIM" "$RESET"
  else
    printf "  %sTarget:%s   %s (%s)\n" "$BOLD" "$RESET" "$HEAD_SHA" "$HEAD_BRANCH"
  fi
  if [[ -n "$PREVIOUS_TAG" ]]; then
    local count
    count="$(printf "%s" "$COMMITS" | grep -c . || true)"
    printf "  %sSince:%s    %s %s(%s commits)%s\n" \
      "$BOLD" "$RESET" "$PREVIOUS_TAG" "$DIM" "$count" "$RESET"
  else
    printf "  %sSince:%s    %s(none — full history)%s\n" \
      "$BOLD" "$RESET" "$DIM" "$RESET"
  fi
  local scope="public"
  [[ "$SHOW_INTERNAL" -eq 1 ]] && scope="internal (includes all sections)"
  printf "  %sScope:%s    %s\n" "$BOLD" "$RESET" "$scope"
  hr "$color"
  printf "\n"
}

style_notes() {
  if [[ "$RAW" -eq 1 ]]; then
    printf "%s" "$1"
    return
  fi
  printf "%s" "$1" | awk \
    -v B="$BOLD" -v D="$DIM" -v C="$CYAN" -v M="$MAGENTA" -v G="$GREEN" -v R="$RESET" '
    /^## / {
      sub(/^## /, "")
      printf "%s%s%s%s\n\n", B, C, $0, R
      next
    }
    /^### / {
      sub(/^### /, "")
      printf "%s%s%s%s\n", B, M, $0, R
      next
    }
    /^[*-] / {
      sub(/^[*-] /, "")
      printf "  %s•%s %s\n", G, R, $0
      next
    }
    NF == 0 { print ""; next }
    { print }
  '
}

print_header
style_notes "$NOTES"
printf "\n"
