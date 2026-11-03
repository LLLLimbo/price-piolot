#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" != "--no-build" ]; then
  docker compose build
  docker compose -f tests/compose.yaml build go-tests
  docker build --build-arg HTTP_PROXY --build-arg HTTPS_PROXY -t price-pilot-browser:local -f tests/Dockerfile.browser .
fi
mkdir -p tests/artifacts
cleanup() { docker compose -f tests/compose.yaml down --volumes --remove-orphans; }
trap cleanup EXIT INT TERM
docker compose -f tests/compose.yaml up -d --wait app app-postgres
docker compose -f tests/compose.yaml run --rm go-tests
docker compose -f tests/compose.yaml run --rm api-tests
docker compose -f tests/compose.yaml run --rm api-tests python /tests/services.py
docker compose -f tests/compose.yaml run --rm api-tests python /tests/profit_checks.py
docker compose -f tests/compose.yaml run --rm api-tests python /tests/persistence.py capture
docker compose -f tests/compose.yaml up -d --no-deps --force-recreate --wait app app-postgres
docker compose -f tests/compose.yaml run --rm api-tests python /tests/persistence.py verify
docker compose -f tests/compose.yaml run --rm browser-tests
docker compose -f tests/compose.yaml run --rm browser-tests node /tests/services-browser.cjs
docker compose -f tests/compose.yaml run --rm browser-tests node /tests/notes-browser.cjs
docker compose -f tests/compose.yaml run --rm browser-tests node /tests/profit-check-browser.cjs
