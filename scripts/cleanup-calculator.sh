#!/usr/bin/env bash
# Removes the calculator demo project from claimd.
# Tasks, events, and all project metadata are deleted together.
# Usage: ./scripts/cleanup-calculator.sh [--dir <claimd-dir>]

set -euo pipefail

CLAIMD="${CLAIMD_BIN:-claimd}"
PROJECT="calculator"
DIR_FLAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) DIR_FLAG="--dir $2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

$CLAIMD $DIR_FLAG project remove "$PROJECT"
