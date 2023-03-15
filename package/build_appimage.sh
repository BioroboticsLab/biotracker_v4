#!/bin/bash

cd "$(dirname "$0")"
source scl_source enable llvm-toolset-7.0
export PROTOC=/opt/protoc/bin/protoc
export PYLON_ROOT=/opt/pylon5
BUILD_DIR=/tmp/biotracker_build
APPDIR=$BUILD_DIR/AppDir

set -e

mkdir $BUILD_DIR || true
cargo build --release --target-dir $BUILD_DIR --features pylon
rm -rf $APPDIR || true
/opt/linuxdeploy/usr/bin/linuxdeploy \
  -e $BUILD_DIR/release/biotracker4 \
  --appdir $APPDIR \
  --desktop-file resources/biotracker4.desktop \
  --icon-file resources/biotracker4.png \
  --output appimage \
  --exclude-library 'libwayland-*'
mv -- *.AppImage biotracker.AppImage
