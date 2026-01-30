BINARY_NAME=klog

# Default target for local development
build:
	cargo build --release

# --- RELEASE TARGETS ---

# Linux Static (amd64) - Requires musl-tools on the host
build-linux-amd64:
	rustup target add x86_64-unknown-linux-musl
	cargo build --release --target x86_64-unknown-linux-musl
	cp target/x86_64-unknown-linux-musl/release/$(BINARY_NAME) $(BINARY_NAME)-linux-amd64

# Linux Static (arm64/Graviton) - Requires aarch64-linux-musl-gcc
build-linux-arm64:
	rustup target add aarch64-unknown-linux-musl
	# Note: On GitHub Actions, we usually use 'cross' for this to avoid manual linker setup
	cargo install cross
	cross build --release --target aarch64-unknown-linux-musl
	cp target/aarch64-unknown-linux-musl/release/$(BINARY_NAME) $(BINARY_NAME)-linux-arm64

# macOS Intel
build-macos-amd64:
	rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/$(BINARY_NAME) $(BINARY_NAME)-macos-amd64

# macOS M1/M2/M3
build-macos-arm64:
	rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin
	cp target/aarch64-apple-darwin/release/$(BINARY_NAME) $(BINARY_NAME)-macos-arm64

# Windows
build-windows-amd64:
	rustup target add x86_64-pc-windows-msvc
	cargo build --release --target x86_64-pc-windows-msvc
	cp target/x86_64-pc-windows-msvc/release/$(BINARY_NAME).exe $(BINARY_NAME)-windows-amd64.exe

clean:
	cargo clean
	rm -f $(BINARY_NAME)-*