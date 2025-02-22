.PHONY: build-release test build-server-container run-server clean

BINARY_NAME := server
CONTAINER_NAME := multiplayer-bevy-server
CONTAINER_TAG := latest

build-release:
	cargo build --release --locked

release-mac: build-release
	set -e
	strip target/release/$(BINARY_NAME)
	otool -L target/release/$(BINARY_NAME)
	ls -lisah target/release/$(BINARY_NAME)
	mkdir -p release
	tar -C ./target/release/ -czvf ./release/$(BINARY_NAME)-mac.tar.gz ./$(BINARY_NAME)
	ls -lisah ./release/$(BINARY_NAME)-mac.tar.gz

release-mac-x86: build-apple-x86-release
	set -e
	strip target/x86_64-apple-darwin/release/$(BINARY_NAME)
	otool -L target/x86_64-apple-darwin/release/$(BINARY_NAME)
	ls -lisah target/x86_64-apple-darwin/release/$(BINARY_NAME)
	mkdir -p release
	tar -C ./target/x86_64-apple-darwin/release/ -czvf ./release/$(BINARY_NAME)-mac-x86.tar.gz ./$(BINARY_NAME)
	ls -lisah ./release/$(BINARY_NAME)-mac-x86.tar.gz

build-apple-x86-debug:
	cargo build --target=x86_64-apple-darwin

build-apple-x86-release:
	cargo build --release --target=x86_64-apple-darwin --locked

test:
	cargo test

build-server-container:
	docker build -f server.Dockerfile -t $(CONTAINER_NAME):$(CONTAINER_TAG) .

run-server-container:
	docker run -it --rm -p 5000:5000/tcp -p 5000:5000/udp $(CONTAINER_NAME):$(CONTAINER_TAG)

clean:
	cargo clean
