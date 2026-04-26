.PHONY: build build-core build-server test test-core test-server clean fmt lint help

BIN_DIR := bin
CORE_BIN := codenexus-core
SERVER_BIN := codenexus
EMBED_DIR := server/embed

help:
	@echo "CodeNexus build entry. Phase -1 / 0 will refine."
	@echo "  make build         - build core (Rust) + server (Go), embed core into server"
	@echo "  make build-core    - cargo build --release in core/"
	@echo "  make build-server  - go build in server/, requires core built first"
	@echo "  make test          - run core (cargo test) + server (go test) tests"
	@echo "  make fmt           - cargo fmt + gofmt"
	@echo "  make lint          - cargo clippy + go vet"
	@echo "  make clean         - cargo clean + go clean + remove bin/"

build: build-core build-server

build-core:
	cd core && cargo build --release

build-server: build-core
	mkdir -p $(EMBED_DIR)
	cp core/target/release/$(CORE_BIN)$(if $(filter Windows_NT,$(OS)),.exe,) $(EMBED_DIR)/
	mkdir -p $(BIN_DIR)
	cd server && go build -o ../$(BIN_DIR)/$(SERVER_BIN)$(if $(filter Windows_NT,$(OS)),.exe,) .

test: test-core test-server

test-core:
	cd core && cargo test

test-server:
	cd server && go test ./...

fmt:
	cd core && cargo fmt
	cd server && gofmt -w .

lint:
	cd core && cargo clippy -- -D warnings
	cd server && go vet ./...

clean:
	cd core && cargo clean
	cd server && go clean
	rm -rf $(BIN_DIR) $(EMBED_DIR)
