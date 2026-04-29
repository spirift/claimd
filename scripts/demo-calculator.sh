#!/usr/bin/env bash
# Creates a calculator TypeScript/Vite project in claimd with a realistic task graph.
# Usage: ./scripts/demo-calculator.sh [--dir <claimd-dir>]

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

C="$CLAIMD $DIR_FLAG --project $PROJECT"
JSON="$CLAIMD $DIR_FLAG --project $PROJECT --json"

id_of() {
  python3 -c "import sys,json; print(json.load(sys.stdin)['id'][:8])"
}

echo "Initialising project '$PROJECT'..."
$C init

echo "Adding tasks..."

SETUP=$($JSON add "Project setup: TypeScript Vite scaffold" \
  --desc "Scaffold a new Vite project with TypeScript, configure tsconfig, install deps" \
  --priority 0 \
  --tag setup | id_of)
echo "  setup:    $SETUP"

ADD=$($JSON add "Implement add function" \
  --desc "Pure function: (a: number, b: number) => number. Unit tests required." \
  --priority 1 \
  --tag math \
  --depends-on "$SETUP" | id_of)
echo "  add:      $ADD"

SUB=$($JSON add "Implement subtract function" \
  --desc "Pure function: (a: number, b: number) => number. Unit tests required." \
  --priority 1 \
  --tag math \
  --depends-on "$SETUP" | id_of)
echo "  subtract: $SUB"

MUL=$($JSON add "Implement multiply function" \
  --desc "Pure function: (a: number, b: number) => number. Unit tests required." \
  --priority 1 \
  --tag math \
  --depends-on "$SETUP" | id_of)
echo "  multiply: $MUL"

DIV=$($JSON add "Implement divide function" \
  --desc "Pure function: (a: number, b: number) => number. Throws on divide-by-zero. Unit tests required." \
  --priority 1 \
  --tag math \
  --depends-on "$SETUP" | id_of)
echo "  divide:   $DIV"

UI=$($JSON add "Build calculator UI" \
  --desc "React component with display, digit buttons (0-9), operator buttons (+/-/*/÷), equals, and clear" \
  --priority 2 \
  --tag ui \
  --depends-on "$ADD" \
  --depends-on "$SUB" \
  --depends-on "$MUL" \
  --depends-on "$DIV" | id_of)
echo "  ui:       $UI"

WIRE=$($JSON add "Wire UI to math functions" \
  --desc "Connect button handlers to math module. E2E tests for the full calculation flow." \
  --priority 3 \
  --tag integration \
  --depends-on "$UI" | id_of)
echo "  wire:     $WIRE"

echo ""
echo "Done. Task graph:"
$C list --all
echo ""
echo "Events so far:"
$C events list
