# Construit à deux endroits, et c'est voulu :
#
#   - sur chaque pull request par GitHub Actions, pour qu'un Dockerfile cassé
#     échoue sur la PR plutôt qu'au moment du déploiement ;
#   - par Heroku lui-même au déploiement, d'après `heroku.yml`.
#
# Il doit rester à la racine. Heroku fixe le contexte de build au dossier qui
# contient le Dockerfile et ne permet pas de le configurer : rangé dans
# `server/`, il ne pourrait pas copier `web/` et le site ne serait pas
# construit.
#
# Le site de présentation est construit ici et copié dans l'image finale : un
# seul dyno sert le site et l'API, sur un seul domaine, pour un seul prix.
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
