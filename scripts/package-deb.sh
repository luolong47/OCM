#!/usr/bin/env bash
set -euo pipefail
# Build OCM .deb: single binary with embedded frontend + desktop entry

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
VERSION="${1:-0.1.0}"
ARCH="$(dpkg --print-architecture 2>/dev/null || uname -m)"
DEB_DIR="$(mktemp -d)"
trap 'rm -rf "$DEB_DIR"' EXIT

cd "$REPO_DIR/frontend"
pnpm install --frozen-lockfile
pnpm build

cd "$REPO_DIR/backend"
cargo build --release

mkdir -p "$DEB_DIR/usr/bin"
cp "$REPO_DIR/backend/target/release/ocm-backend" "$DEB_DIR/usr/bin/ocm-backend"

mkdir -p "$DEB_DIR/usr/share/applications"
cat > "$DEB_DIR/usr/share/applications/ocm.desktop" <<-DESKTOP
[Desktop Entry]
Name=OCM
Comment=OpenCode Config Manager
Exec=/usr/bin/ocm-backend
Type=Application
Terminal=false
Categories=Utility;
DESKTOP

mkdir -p "$DEB_DIR/DEBIAN"
cat > "$DEB_DIR/DEBIAN/control" <<-CONTROL
Package: ocm
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: $(git config user.name 2>/dev/null || echo 'OCM Developer') <$(git config user.email 2>/dev/null || echo 'dev@example.com')>
Description: OpenCode Config Manager
 Single binary with embedded web UI.
CONTROL

fakeroot dpkg-deb --build "$DEB_DIR" "$REPO_DIR/ocm_${VERSION}_${ARCH}.deb" 2>/dev/null \
  || dpkg-deb --build "$DEB_DIR" "$REPO_DIR/ocm_${VERSION}_${ARCH}.deb"

echo "Package: $(ls -lh "$REPO_DIR/ocm_${VERSION}_${ARCH}.deb")"
