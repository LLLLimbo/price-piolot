#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
docker run --rm -e HTTP_PROXY -e HTTPS_PROXY \
  -v "$PWD:/app" -v price-pilot-go-mod:/go/pkg/mod -v price-pilot-go-cache:/root/.cache/go-build \
  -w /app golang:1.26 sh -c 'go test ./internal/... ./cmd/... -count=1 && go vet ./internal/... ./cmd/...'
docker run --rm -v "$PWD/web:/app" -w /app node:22-bookworm \
  sh -c 'npm run typecheck && npm test && npm run build'
