#!/bin/bash

# Poratake Delete Release Script
# This script deletes a release and rolls back all associated changes
#
# Usage:
#   ./scripts/delete-release.sh <version> [previous-version]
#
# Arguments:
#   version           - The version to delete (required)
#   previous-version  - The version to roll back to (optional, auto-detected if not provided)
#
# Examples:
#   ./scripts/delete-release.sh 1.1.0 1.0.0
#   ./scripts/delete-release.sh 0.28.0 0.27.0

set -e

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

# Parse arguments
VERSION="$1"
PREVIOUS_VERSION="$2"

# Validate version
if [[ -z "$VERSION" ]]; then
  log_error "Version is required"
  echo "Usage: $0 <version> [previous-version]"
  echo "Example: $0 1.1.0 1.0.0"
  exit 1
fi

if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  log_error "Version must be in format X.Y.Z (e.g., 1.1.0)"
  exit 1
fi

# Validate previous version format if provided
if [[ -n "$PREVIOUS_VERSION" ]] && ! echo "$PREVIOUS_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  log_error "Previous version must be in format X.Y.Z (e.g., 1.0.0)"
  exit 1
fi

log_info "Deleting release v$VERSION"
echo ""

# Confirm with user
echo -e "${YELLOW}This will:${NC}"
echo "  1. Delete GitHub release v$VERSION (if exists)"
echo "  2. Delete remote tag v$VERSION (if exists)"
echo "  3. Delete local tag v$VERSION (if exists)"
if [[ -n "$PREVIOUS_VERSION" ]]; then
  echo "  4. Update package.json to v$PREVIOUS_VERSION"
else
  echo "  4. Update package.json to previous version (auto-detected)"
fi
echo ""
read -p "Are you sure you want to continue? (y/N) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  log_info "Aborted"
  exit 0
fi

echo ""

# Step 1: Delete GitHub release
log_info "Step 1: Checking for GitHub release..."
if gh release view "v$VERSION" &>/dev/null; then
  log_info "Deleting GitHub release v$VERSION..."
  gh release delete "v$VERSION" --yes
  log_success "GitHub release deleted"
else
  log_warning "GitHub release v$VERSION not found (skipping)"
fi
echo ""

# Step 2: Delete remote tag
log_info "Step 2: Checking for remote tag..."
if git ls-remote --tags origin | grep -q "refs/tags/v$VERSION"; then
  log_info "Deleting remote tag v$VERSION..."
  git push --delete origin "v$VERSION"
  log_success "Remote tag deleted"
else
  log_warning "Remote tag v$VERSION not found (skipping)"
fi
echo ""

# Step 3: Delete local tag
log_info "Step 3: Checking for local tag..."
if git tag -l | grep -q "^v$VERSION$"; then
  log_info "Deleting local tag v$VERSION..."
  git tag -d "v$VERSION"
  log_success "Local tag deleted"
else
  log_warning "Local tag v$VERSION not found (skipping)"
fi
echo ""

# Step 4: Update package.json version
log_info "Step 4: Updating package.json version..."
CURRENT_VERSION=$(jq -r '.version' package.json)

if [[ "$CURRENT_VERSION" != "$VERSION" ]]; then
  log_warning "Current version ($CURRENT_VERSION) doesn't match target ($VERSION)"
  log_info "Package.json may have already been reverted or updated"
else
  # Determine the target version to roll back to
  TARGET_VERSION="$PREVIOUS_VERSION"

  if [[ -z "$TARGET_VERSION" ]]; then
    # Auto-detect previous version from tags
    log_info "Auto-detecting previous version from tags..."
    PREVIOUS_TAG=$(git describe --tags --abbrev=0 "v$VERSION^" 2>/dev/null || echo "")

    if [[ -n "$PREVIOUS_TAG" ]]; then
      TARGET_VERSION="${PREVIOUS_TAG#v}"
      log_info "Found previous tag: $PREVIOUS_TAG"
    else
      log_error "Could not auto-detect previous version. Please provide it as a second argument:"
      echo "  $0 $VERSION <previous-version>"
      exit 1
    fi
  fi

  log_info "Updating package.json from v$VERSION to v$TARGET_VERSION..."
  jq --arg v "$TARGET_VERSION" '.version = $v' package.json > package.json.tmp
  mv package.json.tmp package.json
  git add package.json
  git commit -m "chore: revert version to $TARGET_VERSION after deleting release v$VERSION"
  git push
  log_success "package.json updated to v$TARGET_VERSION"
fi
echo ""

# Step 5: Clean up local build artifacts (optional)
RELEASE_DIR="release/$VERSION"
if [[ -d "$RELEASE_DIR" ]]; then
  log_info "Step 5: Found local build artifacts..."
  read -p "Delete local build artifacts in $RELEASE_DIR? (y/N) " -n 1 -r
  echo ""
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$RELEASE_DIR"
    log_success "Local build artifacts deleted"
  else
    log_info "Keeping local build artifacts"
  fi
else
  log_info "Step 5: No local build artifacts found"
fi
echo ""

log_success "Release v$VERSION deletion completed!"
echo ""
echo "Summary:"
echo "  - GitHub release: deleted"
echo "  - Remote tag: deleted"
echo "  - Local tag: deleted"
FINAL_VERSION=$(jq -r '.version' package.json)
echo "  - Current package.json version: $FINAL_VERSION"
