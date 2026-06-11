#!/bin/bash
# Re-vendor near-contract-standards from upstream, preserving local modifications.
#
# Source of truth is the vendor directory itself (`vendors/near-contract-standards/`).
# Local modifications are derived on the fly by diffing the live vendor against the
# pristine upstream tarball at the version it was based on. After the target upstream
# version is dropped in, that diff is re-applied.
#
# Workflow:
#   1. Read CURRENT version from vendor's Cargo.toml.
#   2. Download pristine CURRENT from crates.io and diff against live vendor — this
#      captures every local modification, regardless of who made it or when.
#   3. Read TARGET version from workspace [workspace.dependencies] near-sdk.
#   4. Download TARGET from crates.io and replace the vendor directory.
#   5. Re-apply the captured diff.
#
# If a hunk fails (upstream changed near our modification site), `patch` writes .rej
# files and aborts. Inspect, resolve manually, and commit the resolved state.
# The next `make sync` will re-derive a fresh diff from your resolved vendor.

set -e

# Run from the workspace root regardless of invocation directory.
cd "$(dirname "$0")/.."

CARGO_TOML="Cargo.toml"
TARGET_DIR="vendors/near-contract-standards"
CRATE="near-contract-standards"

# Target version: what we want to migrate TO (read from workspace deps).
TARGET_VERSION=$(grep -E '^near-sdk\s*=\s*' "$CARGO_TOML" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)
if [ -z "$TARGET_VERSION" ]; then
    echo "❌ Could not find near-sdk version in $CARGO_TOML"
    exit 1
fi

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

download_crate() {
    # Extracts `<crate>-<version>/` into the given directory.
    local version="$1"
    local into="$2"
    mkdir -p "$into"
    curl -fsSL "https://crates.io/api/v1/crates/$CRATE/$version/download" \
        | tar -xz -C "$into"
}

# 1. Capture local modifications as the diff between live vendor and pristine upstream.
LOCAL_PATCH=""
if [ -d "$TARGET_DIR" ]; then
    CURRENT_VERSION=$(grep -E '^version\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"' "$TARGET_DIR/Cargo.toml" \
        | head -n 1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
    if [ -z "$CURRENT_VERSION" ]; then
        echo "❌ Could not determine current vendored version from $TARGET_DIR/Cargo.toml"
        exit 1
    fi
    echo "📦 Currently vendored: $CRATE@$CURRENT_VERSION"

    echo "⬇️  Downloading pristine $CURRENT_VERSION for diff baseline..."
    download_crate "$CURRENT_VERSION" "$TMP_DIR/stage"

    # Stage both trees under the same relative path so diff output has clean prefixes
    # (`base/$TARGET_DIR` and `head/$TARGET_DIR`) without regex-escaping concerns.
    mkdir -p "$TMP_DIR/base/$(dirname "$TARGET_DIR")"
    mv "$TMP_DIR/stage/$CRATE-$CURRENT_VERSION" "$TMP_DIR/base/$TARGET_DIR"
    mkdir -p "$TMP_DIR/head/$(dirname "$TARGET_DIR")"
    cp -r "$TARGET_DIR" "$TMP_DIR/head/$TARGET_DIR"

    echo "📝 Computing local modifications..."
    LOCAL_PATCH="$TMP_DIR/local.patch"
    # diff returns 1 when files differ — expected and not an error.
    ( cd "$TMP_DIR" && diff -ruN "base/$TARGET_DIR" "head/$TARGET_DIR" ) \
        | sed -e "s|^--- base/|--- a/|" -e "s|^+++ head/|+++ b/|" \
        > "$LOCAL_PATCH" || true

    if [ -s "$LOCAL_PATCH" ]; then
        echo "    captured $(grep -c '^@@' "$LOCAL_PATCH") hunk(s) of local mods"
    else
        echo "    no local modifications detected"
        LOCAL_PATCH=""
    fi
fi

# 2. Download the target upstream version.
echo "⬇️  Downloading $CRATE@$TARGET_VERSION from crates.io..."
download_crate "$TARGET_VERSION" "$TMP_DIR/target"

# 3. Replace the vendor directory.
echo "🔄 Replacing $TARGET_DIR with $TARGET_VERSION..."
mkdir -p "$(dirname "$TARGET_DIR")"
rm -rf "$TARGET_DIR"
mv "$TMP_DIR/target/$CRATE-$TARGET_VERSION" "$TARGET_DIR"

# 4. Re-apply local modifications.
if [ -n "$LOCAL_PATCH" ]; then
    echo "🛠️  Re-applying local modifications..."
    patch -p1 < "$LOCAL_PATCH"
fi

echo "🎉 Vendor sync complete!"
