#!/bin/sh
set -e
cd "$(dirname "$0")/biotracker"
PROTO_PATH=$(realpath ../../protocol)
python -m grpc_tools.protoc -I "$PROTO_PATH" --python_betterproto_out=biotracker/ "$PROTO_PATH"/*.proto
