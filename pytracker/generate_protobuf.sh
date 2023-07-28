#!/bin/bash
set -e
SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
PROTO_PATH=$(realpath "${SCRIPT_DIR}/../protocol")
mkdir "$SCRIPT_DIR/biotracker/biotracker" || true
python -m grpc_tools.protoc -I "$PROTO_PATH" --python_betterproto_out="$SCRIPT_DIR/biotracker/biotracker" "$PROTO_PATH"/*.proto
