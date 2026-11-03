FROM golang:1.26 AS go-base
WORKDIR /build
COPY go.mod go.sum ./
RUN go mod download
COPY cmd/ ./cmd/
COPY internal/ ./internal/


FROM node:22-bookworm AS web-deps
WORKDIR /build/web
COPY web/package.json web/package-lock.json web/.npmrc ./
RUN npm ci --ignore-scripts --no-audit --no-fund

FROM web-deps AS web-build
COPY web/ ./
RUN npm run typecheck && npm test && npm run build

FROM go-base AS go-build
RUN CGO_ENABLED=1 go build -trimpath -o /pricepilot ./cmd/server

FROM debian:bookworm-slim AS app
COPY --from=go-build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
WORKDIR /app
COPY --from=go-build /pricepilot ./pricepilot
COPY --from=web-build /build/web/dist ./web/dist
RUN mkdir /app/data && chown -R 65532:65532 /app/data
USER 65532:65532
ENV LISTEN_ADDR=:8080 DATABASE_DRIVER=sqlite DATABASE_DSN=/app/data/pricepilot.db
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --retries=20 CMD ["/app/pricepilot", "health"]
ENTRYPOINT ["/app/pricepilot"]
