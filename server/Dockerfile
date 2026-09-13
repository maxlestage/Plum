# Built in GitHub Actions, never on Heroku: Heroku caps builds at 15 minutes
# and a Rust release build of this blows through that. Heroku only ever
# receives a finished image.

# The presentation site is built here and copied into the final image: one
# dyno serves the site and the API, on one domain, for one price.
FROM node:22-slim AS site
WORKDIR /site
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-slim-bookworm AS builder

WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Dependencies first, against skeleton sources: this layer changes only when
# Cargo.toml does, so day-to-day builds reuse it and take a minute instead of
# fifteen.
COPY server/Cargo.toml server/Cargo.lock ./
COPY server/migration/Cargo.toml migration/Cargo.toml
RUN mkdir -p src migration/src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs migration/src/lib.rs \
    && cargo build --release \
    && rm -rf src migration/src

COPY server/ .
# Cargo keys off mtimes; without this it trusts the skeleton it just built.
RUN touch src/main.rs src/lib.rs migration/src/lib.rs \
    && cargo build --release

FROM debian:bookworm-slim

# rustls verifies certificates against the system store, which a slim image
# does not ship.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 plum

COPY --from=builder /app/target/release/plum-server /usr/local/bin/plum-server
COPY --from=site /site/dist /srv/site

ENV SITE_DIR=/srv/site

USER plum
# No EXPOSE: Heroku assigns $PORT at run time and the process reads it.
CMD ["plum-server"]
