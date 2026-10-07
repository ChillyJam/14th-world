# Builds the server and the WebAssembly client into one small runtime image.
FROM rust:1-bookworm AS build
ARG TRUNK_VERSION=v0.21.14
RUN rustup target add wasm32-unknown-unknown \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin
WORKDIR /app
COPY . .
RUN cd crates/client && trunk build --release
RUN cargo build --release --bin world-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 world && mkdir /data && chown world /data
COPY --from=build /app/target/release/world-server /usr/local/bin/world-server
COPY --from=build /app/crates/client/dist /srv/client
USER world
ENV BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=sqlite:///data/world.db \
    STATIC_DIR=/srv/client
VOLUME /data
EXPOSE 8080
CMD ["world-server"]
