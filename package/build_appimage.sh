#!/bin/bash

cd "$(dirname "$0")"
export PROTOC=/opt/protoc/bin/protoc
export PYLON_ROOT=/opt/pylon5
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:${PYLON_ROOT}/lib64"
BUILD_DIR=/tmp/biotracker_build
APPDIR=$BUILD_DIR/AppDir

set -e

mkdir $BUILD_DIR || true
cargo build --release --target-dir $BUILD_DIR --features pylon
rm -rf $APPDIR || true
rm biotracker.AppImage || true
/opt/linuxdeploy/usr/bin/linuxdeploy \
  -e $BUILD_DIR/release/biotracker4 \
  --appdir $APPDIR \
  --desktop-file resources/biotracker4.desktop \
  --icon-file resources/biotracker4.png \
  --output appimage \
  --exclude-library 'libwayland-*'
mv -- *.AppImage biotracker.AppImage
