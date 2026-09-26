#!/usr/bin/env bash
# Serves a production build of the Tint sandbox (dist/) over plain HTTP.
# The sandbox source uses Vite-resolved CodeMirror/WASM modules, so run the
# production build first. `npm run preview` does the same job; this script is
# the zero-extra-dependency fallback using Python's http.server.
set -euo pipefail
cd "$(dirname "$0")"
if [ ! -d dist ]; then
  echo "dist/ not found -- run 'npm install && npm run build' first." >&2
  exit 1
fi
PORT="${1:-8123}"
echo "Serving Tint sandbox (production build) at http://localhost:$PORT"
cd dist
python3 -m http.server "$PORT"
