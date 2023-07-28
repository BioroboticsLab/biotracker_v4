#!/bin/bash
set -eo pipefail

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
VENV="$SCRIPT_DIR/biotracker-venv"

if [ -d "$VENV" ]; then
    echo "Virtual environment already exists at $VENV"
else
    python3 -m venv "$VENV"
fi
source "$VENV/bin/activate"
pip install -r "$SCRIPT_DIR/requirements.txt"
sh "$SCRIPT_DIR/../../pytracker/generate_protobuf.sh"
