#!/bin/bash
set -e

# Change to the root of the repository
cd "$(dirname "$0")/.."

# 1. Check prerequisites
for cmd in bun cargo gh; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: '$cmd' is required but not installed." >&2
        exit 1
    fi
done

# 2. Extract version from Cargo.toml
VERSION=$(grep '^version = ' Cargo.toml | head -n 1 | awk -F '"' '{print $2}')
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml" >&2
    exit 1
fi
TAG="v$VERSION"
echo "Detected version: $TAG"

# 3. Determine OS and target mapping
OS=$(uname -s)
if [ "$OS" = "Darwin" ]; then
    VENDOR_TARGET="macos-x86_64"
    ARCHIVE_EXT="tar.gz"
elif [ "$OS" = "Linux" ]; then
    VENDOR_TARGET="linux-x86_64"
    ARCHIVE_EXT="tar.gz"
elif echo "$OS" | grep -qi "MINGW\|MSYS\|CYGWIN"; then
    VENDOR_TARGET="windows-x86_64"
    ARCHIVE_EXT="zip"
    if ! command -v 7z >/dev/null 2>&1; then
        echo "Error: '7z' is required on Windows for zipping artifacts." >&2
        exit 1
    fi
else
    echo "Error: Unsupported OS '$OS'" >&2
    exit 1
fi
echo "Target mapped to: $VENDOR_TARGET"

# 4. Build TUI, sidecars, and app server
echo "Building TUI and sidecars..."
bun run build:tui
echo "Building app server..."
cargo build --release -p peregrine-app-server

# 5. Package
echo "Packaging release..."
RELEASE_DIR="peregrine-tui-release"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

if [ "$ARCHIVE_EXT" = "zip" ]; then
    cp target/release/peregrine-tui.exe "$RELEASE_DIR/"
    cp target/release/peregrine-app-server.exe "$RELEASE_DIR/"
    cp target/release/peregrine-helper.exe "$RELEASE_DIR/"
    cp target/release/peregrine-sui-mcp-server.exe "$RELEASE_DIR/"
    cp target/release/peregrine-sui-move-analyzer-mcp-server.exe "$RELEASE_DIR/"
    cp target/release/peregrine-sui-move-knowledge.exe "$RELEASE_DIR/"
else
    cp target/release/peregrine-tui "$RELEASE_DIR/"
    cp target/release/peregrine-app-server "$RELEASE_DIR/"
    cp target/release/peregrine-helper "$RELEASE_DIR/"
    cp target/release/peregrine-sui-mcp-server "$RELEASE_DIR/"
    cp target/release/peregrine-sui-move-analyzer-mcp-server "$RELEASE_DIR/"
    cp target/release/peregrine-sui-move-knowledge "$RELEASE_DIR/"
fi

ARTIFACT_NAME="peregrine-tui-$VENDOR_TARGET"

cd "$RELEASE_DIR"
if [ "$ARCHIVE_EXT" = "zip" ]; then
    7z a "../$ARTIFACT_NAME.zip" *
    cd ..
    sha256sum "$ARTIFACT_NAME.zip" > "$ARTIFACT_NAME.zip.sha256"
else
    tar -czvf "../$ARTIFACT_NAME.tar.gz" *
    cd ..
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$ARTIFACT_NAME.tar.gz" > "$ARTIFACT_NAME.tar.gz.sha256"
    else
        shasum -a 256 "$ARTIFACT_NAME.tar.gz" > "$ARTIFACT_NAME.tar.gz.sha256"
    fi
fi

# 6. Publish to GitHub
echo "Publishing release $TAG to GitHub..."
if gh release view "$TAG" >/dev/null 2>&1; then
    echo "Release $TAG already exists. Uploading assets..."
    gh release upload "$TAG" "$ARTIFACT_NAME.$ARCHIVE_EXT" "$ARTIFACT_NAME.$ARCHIVE_EXT.sha256" --clobber
else
    echo "Creating new release $TAG..."
    gh release create "$TAG" "$ARTIFACT_NAME.$ARCHIVE_EXT" "$ARTIFACT_NAME.$ARCHIVE_EXT.sha256" --title "Peregrine $TAG" --generate-notes
fi

echo "Clean up..."
rm -rf "$RELEASE_DIR"
echo "Done!"
