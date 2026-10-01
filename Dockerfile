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
#
# Deux étages Rust, et non un seul : le site vise `wasm32-unknown-unknown` et
# le serveur vise la machine. Les mêler ferait recompiler l'un à chaque
# changement de l'autre.
# Le site est en Rust, compilé en WebAssembly par Trunk, et pré-rendu en
# treize pages par un second binaire du même crate.
#
# Trunk, wasm-bindgen et wasm-opt viennent en binaires déjà construits plutôt
# que par `cargo install` : les compiler ici coûterait plusieurs minutes, et
# Heroku plafonne un build à quinze. Le déploiement mesuré tenait en trois
# minutes avant ce changement ; c'est le budget qu'on dépense.
FROM rust:1-slim-bookworm AS site

WORKDIR /site
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && curl -sSL https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-x86_64-unknown-linux-gnu.tar.gz \
       | tar -xzf - -C /usr/local/bin trunk

# Les dépendances d'abord, contre des sources squelettes : cette couche ne
# change que quand `Cargo.toml` change, donc les constructions ordinaires la
# réutilisent. Les deux cibles sont chauffées — le wasm et l'hôte — parce que
# ce sont deux arbres de dépendances distincts.
COPY web/Cargo.toml web/Cargo.lock ./
RUN mkdir -p src/bin \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > src/bin/prerender.rs \
    && touch src/lib.rs \
    && cargo build --release --target wasm32-unknown-unknown --features hydration --bin plum-site \
    && cargo build --release --features ssr --bin prerender \
    && rm -rf src

COPY web/ ./
# Cargo se fie aux dates : sans ça, il croit les squelettes qu'il vient de
# construire.
RUN touch src/main.rs src/lib.rs src/bin/prerender.rs \
    && trunk build --release --features hydration \
    && cargo run --release --features ssr --bin prerender

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
