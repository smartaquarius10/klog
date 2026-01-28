BINARY_NAME=klog
VERSION=0.1.0

# Help command to list targets
help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  build-linux-amd64   Static binary for Linux (Ubuntu/CentOS/etc)"
	@echo "  build-linux-arm64   Static binary for Linux ARM (AWS Graviton/RasPi)"
	@echo "  build-macos-intel   Binary for Intel Macs"
	@echo "  build-macos-m1      Binary for Apple Silicon (M1/M2/M3)"
	@echo "  build-windows       Binary for Windows (.exe)"
	@echo "  clean               Remove build artifacts"

build-linux-amd64:
	cross build --target x86_64-unknown-linux-musl --release
	cp target/x86_64-unknown-linux-musl/release/$(BINARY_NAME) ./$(BINARY_NAME)-linux-amd64

build-linux-arm64:
	cross build --target aarch64-unknown-linux-musl --release
	cp target/aarch64-unknown-linux-musl/release/$(BINARY_NAME) ./$(BINARY_NAME)-linux-arm64

build-macos-intel:
	cross build --target x86_64-apple-darwin --release
	cp target/x86_64-apple-darwin/release/$(BINARY_NAME) ./$(BINARY_NAME)-macos-intel

build-macos-m1:
	cross build --target aarch64-apple-darwin --release
	cp target/aarch64-apple-darwin/release/$(BINARY_NAME) ./$(BINARY_NAME)-macos-m1

build-windows:
	cross build --target x86_64-pc-windows-gnu --release
	cp target/x86_64-pc-windows-gnu/release/$(BINARY_NAME).exe ./$(BINARY_NAME).exe

clean:
	cargo clean
	rm -f $(BINARY_NAME)-*