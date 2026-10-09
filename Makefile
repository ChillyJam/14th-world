# Build and run the world server, which serves the web client.
#
#   make build       build the server
#   make run         build and run the server, then open http://localhost:8080
#
# Builds are optimised by default. Pass PROFILE=dev for faster, unoptimised
# builds while iterating. On Windows, use make.cmd instead, which has the same
# targets and needs no POSIX shell.

PROFILE ?= release

.PHONY: build run clean

build:
	cargo build --profile $(PROFILE) --bin world-server

run:
	cargo run --profile $(PROFILE) --bin world-server

clean:
	cargo clean
