# Build and run the world server and desktop client.
#
#   make build                         build both binaries
#   make run                           build, then run the server and a client
#   make run-server                    run only the server
#   make run-client SERVER=host:port   run only the client, against any server
#
# Builds are optimised by default. Pass PROFILE=dev for faster, unoptimised
# builds while iterating. On Windows, use make.cmd instead, which has the same
# targets and needs no POSIX shell.

PROFILE ?= release
SERVER ?=

BIN_DIR := target/$(if $(filter dev,$(PROFILE)),debug,$(PROFILE))

.PHONY: build run run-server run-client clean

build:
	cargo build --profile $(PROFILE) --bin world-server --bin world-client

# The client starts in the background and reconnects until the server is up.
# The server stays in the foreground so Ctrl+C reaches it and it saves the
# world before exiting. Closing the client window leaves the server running.
run: build
	$(BIN_DIR)/world-client &
	$(BIN_DIR)/world-server

run-server:
	cargo run --profile $(PROFILE) --bin world-server

run-client:
	cargo run --profile $(PROFILE) --bin world-client -- $(SERVER)

clean:
	cargo clean
