#!/bin/bash

# Poratake Local Release Script
# This script replicates the GitHub Actions release workflow for local execution
# 
# Required environment variables for notarization (set in .env.release or export):
#   APPLE_ID                    - Your Apple ID email
#   APPLE_APP_SPECIFIC_PASSWORD - App-specific password from Apple ID
#   APPLE_TEAM_ID               - Apple Developer Team ID
#
# Optional (only needed in CI, not on your local Mac with certs installed):
#   MACOS_CERTIFICATE           - Base64-encoded .p12 certificate
#   MACOS_CERTIFICATE_PWD       - Password for the .p12 certificate
#
# For GitHub release upload:
#   GH_TOKEN                    - GitHub token with release permissions
#
# Usage:
#   ./scripts/release.sh <version> [--no-notarize] [--skip-upload] [--force-build]
#
# Examples:
#   ./scripts/release.sh 1.1.0
#   ./scripts/release.sh 1.1.0 --no-notarize --skip-upload
#   ./scripts/release.sh 1.1.0 --skip-upload
#   ./scripts/release.sh 1.1.0 --force-build    # Re-build even if artifacts exist

set -eo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Change to project root
cd "$PROJECT_ROOT"

# Parse arguments
VERSION=""
NOTARIZE=true
SKIP_UPLOAD=false
FORCE_BUILD=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --no-notarize)
      NOTARIZE=false
      shift
      ;;
    --skip-upload)
      SKIP_UPLOAD=true
      shift
      ;;
    --force-build)
      FORCE_BUILD=true
      shift
      ;;
    *)
      if [[ -z "$VERSION" ]]; then
        VERSION="$1"
      fi
      shift
      ;;
  esac
done

# Helper functions
log_info() {
  echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
  echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
  echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

# Load environment variables from .env.release if it exists
if [[ -f "$PROJECT_ROOT/.env.release" ]]; then
  log_info "Loading environment from .env.release"
  set -a
  source "$PROJECT_ROOT/.env.release"
  set +a
fi

# Validate version format
if [[ -z "$VERSION" ]]; then
  log_error "Version is required"
  echo "Usage: $0 <version> [--no-notarize] [--skip-upload] [--force-build]"
  echo "Example: $0 1.1.0"
  exit 1
fi

if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  log_error "Version must be in format X.Y.Z (e.g., 1.1.0)"
  exit 1
fi

if [[ "$NOTARIZE" == "false" ]] && [[ "$SKIP_UPLOAD" == "false" ]]; then
  log_error "Non-notarized builds cannot be uploaded"
  exit 1
fi

log_info "Starting release process for version $VERSION"
echo ""

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
  log_error "Release requires a clean working tree"
  exit 1
fi

# Check required environment variables for notarization
if [[ "$NOTARIZE" == "true" ]]; then
  log_info "Checking notarization requirements..."
  
  MISSING_VARS=()
  [[ -z "$APPLE_ID" ]] && MISSING_VARS+=("APPLE_ID")
  [[ -z "$APPLE_APP_SPECIFIC_PASSWORD" ]] && MISSING_VARS+=("APPLE_APP_SPECIFIC_PASSWORD")
  [[ -z "$APPLE_TEAM_ID" ]] && MISSING_VARS+=("APPLE_TEAM_ID")
  
  if [[ ${#MISSING_VARS[@]} -gt 0 ]]; then
    log_error "Missing required environment variables for notarization:"
    for var in "${MISSING_VARS[@]}"; do
      echo "  - $var"
    done
    echo ""
    echo "Set these in .env.release or export them manually."
    echo "Or run with --no-notarize --skip-upload to create a local build."
    exit 1
  fi
  
  log_success "Notarization credentials found"
fi

# Check GitHub token for upload
if [[ "$SKIP_UPLOAD" == "false" ]]; then
  if [[ -z "$GH_TOKEN" ]]; then
    log_error "GH_TOKEN is required unless --skip-upload is used"
    exit 1
  fi
fi

# Define artifact paths
DMG_PATH="release/${VERSION}/Poratake-${VERSION}-universal.dmg"
ZIP_PATH="release/${VERSION}/Poratake-${VERSION}-universal-mac.zip"
DMG_NAME="Poratake-${VERSION}-universal.dmg"
ZIP_NAME="Poratake-${VERSION}-universal-mac.zip"
UPDATE_METADATA_PATH="release/${VERSION}/latest-mac.yml"
BLOCKMAP_PATH="${ZIP_PATH}.blockmap"

# Check if build artifacts already exist
SKIP_BUILD=false
if [[ -f "$DMG_PATH" ]] && [[ -f "$ZIP_PATH" ]] && [[ -f "$UPDATE_METADATA_PATH" ]] && [[ -f "$BLOCKMAP_PATH" ]] && [[ "$FORCE_BUILD" == "false" ]] && [[ "$SKIP_UPLOAD" == "true" ]]; then
  log_info "Build artifacts already exist for v$VERSION:"
  echo "  - $DMG_PATH"
  echo "  - $ZIP_PATH"
  log_warning "Skipping build step (use --force-build to rebuild)"
  SKIP_BUILD=true
  echo ""
fi

# Step 1: Install dependencies
log_info "Step 1: Installing dependencies..."
bun install --frozen-lockfile
log_success "Dependencies installed"
echo ""

# Step 2: Update package.json version
log_info "Step 2: Updating package.json version to $VERSION..."
jq --arg v "$VERSION" '.version = $v' package.json > package.json.tmp
mv package.json.tmp package.json
log_success "Version updated in package.json"
echo ""

restore_local_version() {
  if [[ "$SKIP_UPLOAD" == "true" ]]; then
    git restore -- package.json
  fi
}
trap restore_local_version EXIT

if [[ "$SKIP_UPLOAD" == "false" ]]; then
  log_info "Step 3: Committing version update..."
  git add package.json
  if git diff --cached --quiet; then
    log_info "Version is already set to $VERSION"
  else
    git commit -m "chore: bump version to $VERSION"
  fi
  log_success "Version commit created"
else
  log_info "Step 3: Using the version locally without committing"
fi
BUILD_COMMIT=$(git rev-parse HEAD)
echo ""

# Step 5: Generate release notes
log_info "Step 5: Generating release notes..."

# Get the previous tag
PREVIOUS_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")

if [[ -z "$PREVIOUS_TAG" ]]; then
  COMMITS=$(git log --pretty=format:"%s" --no-merges)
else
  COMMITS=$(git log ${PREVIOUS_TAG}..HEAD --pretty=format:"%s" --no-merges)
fi

FEATURES=""
FIXES=""
INTERNAL=""

while IFS= read -r commit; do
  # Skip empty lines
  if [[ -z "$commit" ]]; then
    continue
  fi

  # Check for feature commits
  if echo "$commit" | grep -qiE "^feat(\(.*\))?:"; then
    MSG=$(echo "$commit" | sed -E 's/^feat(\([^)]*\))?:[[:space:]]*//')
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    FEATURES="${FEATURES}- ${MSG}\n"
  # Check for fix commits
  elif echo "$commit" | grep -qiE "^fix(\(.*\))?:"; then
    MSG=$(echo "$commit" | sed -E 's/^fix(\([^)]*\))?:[[:space:]]*//')
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    FIXES="${FIXES}- ${MSG}\n"
  # Collect internal commits (chore, refactor, docs, internal, tech-debt, etc.)
  else
    # Remove common prefixes: chore, refactor, docs, internal, tech-debt, style, test, ci, build, perf
    MSG=$(echo "$commit" | sed -E 's/^(chore|refactor|docs|internal|tech-debt|style|test|ci|build|perf)(\([^)]*\))?:[[:space:]]*//')
    MSG="$(echo "${MSG:0:1}" | tr '[:lower:]' '[:upper:]')${MSG:1}"
    INTERNAL="${INTERNAL}- ${MSG}\n"
  fi
done <<< "$COMMITS"

# Build the release notes (public)
RELEASE_NOTES=""

if [[ -n "$FEATURES" ]]; then
  RELEASE_NOTES="${RELEASE_NOTES}## Features\n${FEATURES}"
fi

if [[ -n "$FIXES" ]]; then
  if [[ -n "$RELEASE_NOTES" ]]; then
    RELEASE_NOTES="${RELEASE_NOTES}\n"
  fi
  RELEASE_NOTES="${RELEASE_NOTES}## Bug Fixes\n${FIXES}"
fi

# If no features or fixes, provide default message
if [[ -z "$RELEASE_NOTES" ]]; then
  RELEASE_NOTES="No changelog provided"
fi

RELEASE_NOTES=$(echo -e "$RELEASE_NOTES" | sed 's/\\n$//')
echo -e "$RELEASE_NOTES" > release_notes.txt

# Build the internal release notes (includes all sections)
INTERNAL_RELEASE_NOTES=""

if [[ -n "$FEATURES" ]]; then
  INTERNAL_RELEASE_NOTES="${INTERNAL_RELEASE_NOTES}## Features\n${FEATURES}"
fi

if [[ -n "$FIXES" ]]; then
  if [[ -n "$INTERNAL_RELEASE_NOTES" ]]; then
    INTERNAL_RELEASE_NOTES="${INTERNAL_RELEASE_NOTES}\n"
  fi
  INTERNAL_RELEASE_NOTES="${INTERNAL_RELEASE_NOTES}## Bug Fixes\n${FIXES}"
fi

if [[ -n "$INTERNAL" ]]; then
  if [[ -n "$INTERNAL_RELEASE_NOTES" ]]; then
    INTERNAL_RELEASE_NOTES="${INTERNAL_RELEASE_NOTES}\n"
  fi
  INTERNAL_RELEASE_NOTES="${INTERNAL_RELEASE_NOTES}## Internal\n${INTERNAL}"
fi

# If no changes at all, provide default message
if [[ -z "$INTERNAL_RELEASE_NOTES" ]]; then
  INTERNAL_RELEASE_NOTES="No changelog provided"
fi

INTERNAL_RELEASE_NOTES=$(echo -e "$INTERNAL_RELEASE_NOTES" | sed 's/\\n$//')
echo -e "$INTERNAL_RELEASE_NOTES" > internal_release_notes.txt

log_success "Release notes generated:"
cat release_notes.txt
echo ""
log_success "Internal release notes generated:"
cat internal_release_notes.txt
echo ""

# Step 6 & 7: Setup certificates and build (skip if artifacts exist)
if [[ "$SKIP_BUILD" == "false" ]]; then
  # Step 6: Setup macOS certificates (only needed in CI with MACOS_CERTIFICATE)
  # On local Mac, certificates are already in the system keychain
  if [[ -n "$MACOS_CERTIFICATE" ]]; then
    log_info "Step 6: Setting up macOS certificates from environment..."
    
    KEYCHAIN_NAME="build.keychain"
    KEYCHAIN_PWD="${MACOS_KEYCHAIN_PWD:-$(openssl rand -base64 32)}"
    
    # Decode and import certificate
    echo "$MACOS_CERTIFICATE" | base64 --decode > certificate.p12
    
    # Delete existing keychain if it exists
    security delete-keychain "$KEYCHAIN_NAME" 2>/dev/null || true
    
    # Create and configure keychain
    security create-keychain -p "$KEYCHAIN_PWD" "$KEYCHAIN_NAME"
    security default-keychain -s "$KEYCHAIN_NAME"
    security unlock-keychain -p "$KEYCHAIN_PWD" "$KEYCHAIN_NAME"
    security import certificate.p12 -k "$KEYCHAIN_NAME" -P "$MACOS_CERTIFICATE_PWD" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PWD" "$KEYCHAIN_NAME"
    
    # Cleanup certificate file
    rm certificate.p12
    
    log_success "Certificates configured from environment"
    echo ""
  else
    log_info "Step 6: Using certificates from system keychain (local Mac)"
    echo ""
  fi

  # Step 7: Build macOS app
  log_info "Step 7: Building macOS app..."
  BUILD_LOG=$(mktemp)

  if [[ "$NOTARIZE" == "true" ]]; then
    log_info "Building with notarization..."
    # Capture output while still displaying it
    bun run build-mac 2>&1 | tee "$BUILD_LOG"
    BUILD_EXIT_CODE=${PIPESTATUS[0]}

    if [[ $BUILD_EXIT_CODE -ne 0 ]]; then
      log_error "Build failed with exit code $BUILD_EXIT_CODE"
      rm -f "$BUILD_LOG"
      exit 1
    fi

    # Check if notarization was skipped
    if grep -q "notarize skipped\|skipped macOS notarization\|notarize.*options were unable to be generated" "$BUILD_LOG"; then
      log_error "Notarization was skipped by electron-builder!"
      log_error "This usually means credentials are invalid or configuration is wrong."
      log_error "Check your APPLE_ID, APPLE_APP_SPECIFIC_PASSWORD, and APPLE_TEAM_ID."
      log_error "If you intentionally want to skip notarization, use --no-notarize --skip-upload"
      rm -f "$BUILD_LOG"
      exit 1
    fi
  else
    log_info "Building without notarization (local build)..."
    bun run build-mac:local 2>&1 | tee "$BUILD_LOG"
    BUILD_EXIT_CODE=${PIPESTATUS[0]}

    if [[ $BUILD_EXIT_CODE -ne 0 ]]; then
      log_error "Build failed with exit code $BUILD_EXIT_CODE"
      rm -f "$BUILD_LOG"
      exit 1
    fi
  fi

  rm -f "$BUILD_LOG"
  log_success "Build completed"
  echo ""
else
  log_info "Step 6: Skipped (using existing build)"
  log_info "Step 7: Skipped (using existing build)"
  echo ""
fi

# Step 8: Create GitHub Release
if [[ "$SKIP_UPLOAD" == "false" ]]; then
  log_info "Step 8: Creating GitHub release..."

  if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    log_error "Working tree changed after the release commit was created"
    exit 1
  fi
  if [[ "$(git rev-parse HEAD)" != "$BUILD_COMMIT" ]]; then
    log_error "Release commit changed during the build"
    exit 1
  fi

  APP_RESOURCES=$(find "release/$VERSION" -type d -path "*/Poratake.app/Contents/Resources" -print -quit)
  if [[ -z "$APP_RESOURCES" ]]; then
    log_error "Packaged Poratake.app resources were not found"
    exit 1
  fi
  APP_PATH=$(dirname "$(dirname "$APP_RESOURCES")")
  codesign --verify --deep --strict --verbose=2 "$APP_PATH"
  xcrun stapler validate "$APP_PATH"
  
  if [[ ! -s "$DMG_PATH" ]] || [[ ! -s "$ZIP_PATH" ]] || [[ ! -s "$UPDATE_METADATA_PATH" ]] || [[ ! -s "$BLOCKMAP_PATH" ]]; then
    log_error "Build artifacts not found:"
    [[ ! -f "$DMG_PATH" ]] && echo "  Missing: $DMG_PATH"
    [[ ! -f "$ZIP_PATH" ]] && echo "  Missing: $ZIP_PATH"
    [[ ! -f "$UPDATE_METADATA_PATH" ]] && echo "  Missing: $UPDATE_METADATA_PATH"
    [[ ! -f "$BLOCKMAP_PATH" ]] && echo "  Missing: $BLOCKMAP_PATH"
    exit 1
  fi
  node scripts/validate-release-assets.mjs \
    "$VERSION" \
    "$DMG_PATH" \
    "$ZIP_PATH" \
    "$UPDATE_METADATA_PATH" \
    "$BLOCKMAP_PATH" \
    "$APP_RESOURCES"

  git push origin HEAD
  if git rev-parse --verify "refs/tags/v$VERSION" >/dev/null 2>&1; then
    LOCAL_TAG_COMMIT=$(git rev-list -n 1 "v$VERSION")
    if [[ "$LOCAL_TAG_COMMIT" != "$BUILD_COMMIT" ]]; then
      log_error "Tag v$VERSION does not point to the release commit"
      exit 1
    fi
  else
    git tag "v$VERSION"
  fi
  git push origin "v$VERSION"
  REMOTE_TAG_COMMIT=$(git ls-remote origin "refs/tags/v$VERSION^{}" | awk '{print $1}')
  if [[ -z "$REMOTE_TAG_COMMIT" ]]; then
    REMOTE_TAG_COMMIT=$(git ls-remote origin "refs/tags/v$VERSION" | awk '{print $1}')
  fi
  if [[ "$REMOTE_TAG_COMMIT" != "$BUILD_COMMIT" ]]; then
    log_error "Remote tag v$VERSION does not point to the release commit"
    exit 1
  fi
  
  # Get file sizes for progress display
  DMG_SIZE_BYTES=$(stat -f%z "$DMG_PATH")
  ZIP_SIZE_BYTES=$(stat -f%z "$ZIP_PATH")
  DMG_SIZE_MB=$(echo "scale=1; $DMG_SIZE_BYTES / 1048576" | bc)
  ZIP_SIZE_MB=$(echo "scale=1; $ZIP_SIZE_BYTES / 1048576" | bc)
  
  if gh release view "v$VERSION" &>/dev/null; then
    log_error "Release v$VERSION already exists"
    exit 1
  fi
  
  # Create release (without assets)
  log_info "Creating release v$VERSION..."
  gh release create "v$VERSION" \
    --verify-tag \
    --draft \
    --title "Poratake v$VERSION" \
    --notes-file release_notes.txt
  
  log_success "Release created"
  
  # Upload assets separately with progress
  log_info "Uploading DMG (${DMG_SIZE_MB} MB)..."
  gh release upload "v$VERSION" "$DMG_PATH" --clobber 2>&1 | while read -r line; do
    echo "  $line"
  done
  log_success "DMG uploaded"
  
  log_info "Uploading ZIP (${ZIP_SIZE_MB} MB)..."
  gh release upload "v$VERSION" "$ZIP_PATH" --clobber 2>&1 | while read -r line; do
    echo "  $line"
  done
  log_success "ZIP uploaded"

  log_info "Uploading update metadata..."
  gh release upload "v$VERSION" "$UPDATE_METADATA_PATH" --clobber
  gh release upload "v$VERSION" "$BLOCKMAP_PATH" --clobber
  log_success "Update metadata uploaded"

  for ASSET_PATH in "$DMG_PATH" "$ZIP_PATH" "$UPDATE_METADATA_PATH" "$BLOCKMAP_PATH"; do
    ASSET_NAME=$(basename "$ASSET_PATH")
    ASSET_SIZE=$(stat -f%z "$ASSET_PATH")
    REMOTE_SIZE=$(gh release view "v$VERSION" --json assets --jq ".assets[] | select(.name == \"$ASSET_NAME\") | .size")
    if [[ "$REMOTE_SIZE" != "$ASSET_SIZE" ]]; then
      log_error "Uploaded asset verification failed: $ASSET_NAME"
      exit 1
    fi
  done

  gh release edit "v$VERSION" --draft=false
  
  log_success "GitHub release created with all assets"
  echo ""
  
else
  log_warning "Skipping GitHub release upload"
fi

# Cleanup (only if we created a build keychain)
if [[ -n "$MACOS_CERTIFICATE" ]]; then
  log_info "Cleaning up keychain..."
  security default-keychain -s login.keychain 2>/dev/null || true
  security delete-keychain build.keychain 2>/dev/null || true
fi

rm -f release_notes.txt internal_release_notes.txt

echo ""
log_success "Release $VERSION completed successfully!"
echo ""
echo "Artifacts:"
echo "  - release/${VERSION}/Poratake-${VERSION}-universal.dmg"
echo "  - release/${VERSION}/Poratake-${VERSION}-universal-mac.zip"
echo "  - release/${VERSION}/latest-mac.yml"
