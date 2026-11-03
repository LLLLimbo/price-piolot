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
docker compose -f tests/compose.yaml run --rm browser-tests
