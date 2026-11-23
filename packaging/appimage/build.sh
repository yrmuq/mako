#!/bin/sh
# Copyright (C) 2026 yrmuq.
# SPDX-License-Identifier: GPL-3.0-only
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
PACK="$ROOT/packaging/appimage"
APPDIR="$PACK/build/AppDir"
CACHE="$PACK/build/cache"
DIST="$ROOT/dist"
mkdir -p "$CACHE" "$DIST"

ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ;;
    *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac
TOOL="$CACHE/appimagetool-x86_64.AppImage"
TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
TOOL_SHA="e8c1169c4af66c3184d7ee110713d4be04befe671fa358e2c1591ee840553b03"
if [ ! -s "$TOOL" ] || ! echo "$TOOL_SHA  $TOOL" | sha256sum -c - >/dev/null 2>&1; then
    curl -fL --retry 3 -o "$TOOL.part" "$TOOL_URL"
    echo "$TOOL_SHA  $TOOL.part" | sha256sum -c -
    mv "$TOOL.part" "$TOOL"
fi
chmod +x "$TOOL"

cd "$ROOT"
cargo build --locked --release
BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/mako"
test -x "$BIN"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
    "$APPDIR/usr/share/mako/languages" \
    "$APPDIR/usr/share/doc/mako"
cp "$BIN" "$APPDIR/usr/bin/mako"
cp "$ROOT"/languages/*.ini "$APPDIR/usr/share/mako/languages/"
cp "$ROOT/LICENSE" "$ROOT/NOTICE" "$ROOT/README.md" "$PACK/BUNDLED.txt" \
    "$APPDIR/usr/share/doc/mako/"
cp "$ROOT/ui/icons/COPYING" "$APPDIR/usr/share/doc/mako/Breeze-LICENSE.txt"

# The display libraries stay on the host. A bundled libwayland would not match the compositor.
ldd "$BIN" | awk '/=>/ { print $3 }' | while read -r lib; do
    [ -f "$lib" ] || continue
    base=$(basename "$lib")
    case "$base" in
        libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|libutil.so*|ld-linux*) continue ;;
        libgcc_s.so*|libstdc++.so*) continue ;;
        libwayland-*.so*|libxkbcommon*.so*) continue ;;
    esac
    cp -L "$lib" "$APPDIR/usr/lib/$base"
done

cp "$PACK/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"
cp "$ROOT/packaging/io.github.yrmuq.mako.desktop" "$APPDIR/mako.desktop"
cp "$APPDIR/mako.desktop" "$APPDIR/usr/share/applications/io.github.yrmuq.mako.desktop"
cp "$ROOT/assets/icon.svg" "$APPDIR/mako.svg"
cp "$ROOT/assets/icon.png" "$APPDIR/.DirIcon"
cp "$APPDIR/mako.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/mako.svg"

OUT="$DIST/mako-$ARCH.AppImage"
cd "$DIST"
set -- "mako-$ARCH.AppImage"
if [ "${MAKO_DEV_DRAFT:-false}" = true ]; then
    rm -f "$1.zsync"
    APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "$TOOL" --no-appstream "$APPDIR" "$OUT"
else
    APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "$TOOL" --no-appstream \
        -u "gh-releases-zsync|yrmuq|mako|latest|mako-$ARCH.AppImage.zsync" "$APPDIR" "$OUT"
    set -- "$1" "$1.zsync"
fi
chmod +x "$OUT"
sha256sum "$@" > "mako-$ARCH.AppImage.sha256"
