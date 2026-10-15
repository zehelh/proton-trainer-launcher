#!/usr/bin/env bash
# Builds the proton-trainer AppImage.
#
# Prerequisites: a release build at target/release/proton-trainer
# (run `cargo build --release --workspace` first).
#
# Usage: packaging/appimage/build.sh [version]
#
# The script is used by both the regular CI (to keep the packaging
# working on every push) and the release workflow (to publish the
# artifact). It needs no FUSE: linuxdeploy runs in extract-and-run
# mode, so it works on CI runners without /dev/fuse.
set -euo pipefail

VERSION="${1:-0.0.0-dev}"
ARCH=x86_64
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${ROOT}/build"
APPDIR="${BUILD_DIR}/AppDir"
OUTPUT="${BUILD_DIR}/proton-trainer-${VERSION}-${ARCH}.AppImage"

if [[ ! -x "${ROOT}/target/release/proton-trainer" ]]; then
    echo "error: run 'cargo build --release --workspace' first" >&2
    exit 1
fi

rm -rf "${APPDIR}"
mkdir -p \
    "${APPDIR}/usr/bin" \
    "${APPDIR}/usr/share/proton-trainer/trainers" \
    "${APPDIR}/usr/share/icons/hicolor/128x128/apps"

cp "${ROOT}/target/release/proton-trainer" "${APPDIR}/usr/bin/"
cp "${ROOT}"/trainers/*.toml "${APPDIR}/usr/share/proton-trainer/trainers/"
cp "${ROOT}/packaging/appimage/proton-trainer.png" \
    "${APPDIR}/usr/share/icons/hicolor/128x128/apps/proton-trainer.png"
cp "${ROOT}/packaging/appimage/proton-trainer.desktop" "${APPDIR}/"
cp "${ROOT}/packaging/appimage/AppRun" "${APPDIR}/AppRun"
chmod +x "${APPDIR}/AppRun"

# linuxdeploy bundles the AppDir into the AppImage. APPIMAGE_EXTRACT_AND_RUN
# avoids the FUSE requirement on CI runners.
export APPIMAGE_EXTRACT_AND_RUN=1
LINUXDEPLOY="${BUILD_DIR}/linuxdeploy-${ARCH}.AppImage"
wget --quiet \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage" \
    -O "${LINUXDEPLOY}"
chmod +x "${LINUXDEPLOY}"

export OUTPUT VERSION
"${LINUXDEPLOY}" \
    --appdir "${APPDIR}" \
    --desktop-file "${APPDIR}/proton-trainer.desktop" \
    --icon-file "${APPDIR}/usr/share/icons/hicolor/128x128/apps/proton-trainer.png" \
    --output appimage

echo "AppImage written to ${OUTPUT}"
