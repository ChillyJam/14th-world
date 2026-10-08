# Builds the headless world server into one small runtime image. The desktop
# client is built separately and connects to it over WebSocket.
FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release --bin world-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 world && mkdir /data && chown world /data
COPY --from=build /app/target/release/world-server /usr/local/bin/world-server
USER world
ENV BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=sqlite:///data/world.db
VOLUME /data
EXPOSE 8080
CMD ["world-server"]
