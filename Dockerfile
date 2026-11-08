# Binaire statique (musl : static-pie par défaut) dans une image vide : rien d'autre que /ha-mcp (racines TLS
# embarquées).
FROM rust:1-alpine AS build
RUN apk add --no-cache build-base
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY presets presets
COPY test test
RUN cargo build --release --locked && cp target/release/ha-mcp /ha-mcp

FROM scratch
COPY --from=build /ha-mcp /ha-mcp
USER 65534:65534
# mode http : écouter hors du conteneur (stdio, le défaut, n'en a pas besoin)
ENV HA_MCP_ECOUTE=0.0.0.0:8765
EXPOSE 8765
ENTRYPOINT ["/ha-mcp"]
