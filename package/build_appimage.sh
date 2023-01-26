#!/bin/bash

source scl_source enable llvm-toolset-7.0
export PROTOC=/opt/protoc/bin/protoc

set -e

rm -rf AppDir
cargo build --release
/opt/linuxdeploy/usr/bin/linuxdeploy \
  -e target/release/biotracker4 \
  --appdir AppDir \
  --desktop-file package/resources/biotracker4.desktop \
  --icon-file package/resources/biotracker4.png \
  --output appimage \
  --exclude-library 'libwayland-*'
