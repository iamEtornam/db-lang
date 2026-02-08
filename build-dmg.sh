#!/bin/bash
set -e

# Unset CI environment variable that causes issues
unset CI

echo "Building macOS DMG for Query Studio..."
echo ""

cd "$(dirname "$0")"

echo "✓ Frontend already built in dist/"
echo ""

echo "Building Rust backend in release mode..."
cd src-tauri
cargo build --release
echo "✓ Rust backend built"
echo ""

echo "Creating macOS bundles..."
cd ..
npx @tauri-apps/cli bundle --bundles app,dmg

echo ""
echo "✓ Build complete!"
echo ""
echo "Bundles created at:"
echo "  App:  src-tauri/target/release/bundle/macos/db-lang.app"
echo "  DMG:  src-tauri/target/release/bundle/dmg/"
ls -lh src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null || echo "  (DMG creation may have failed)"
