#!/bin/sh
set -eu
export APPIMAGE_EXTRACT_AND_RUN=1
ARCH=$(uname -m)
dist/mako-$ARCH.AppImage --help
(cd dist && sha256sum -c mako-$ARCH.AppImage.sha256)
if [ "${MAKO_DEV_DRAFT:-false}" = true ]; then
    test ! -e "dist/mako-$ARCH.AppImage.zsync"
    update_info=$(env -u APPIMAGE_EXTRACT_AND_RUN dist/mako-$ARCH.AppImage --appimage-updateinformation || true)
    test -z "$update_info"
    exit 0
fi
test -s "dist/mako-$ARCH.AppImage.zsync"
env -u APPIMAGE_EXTRACT_AND_RUN dist/mako-$ARCH.AppImage --appimage-updateinformation | \
    grep -Fx "gh-releases-zsync|yrmuq|mako|latest|mako-$ARCH.AppImage.zsync"
