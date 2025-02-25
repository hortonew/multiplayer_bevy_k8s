# Ran if no arguments provided
default:
  just --list

BINARY_NAME := "server"
CONTAINER_NAME := "multiplayer-bevy-server"
CONTAINER_TAG := "latest"

# build with release and locked flag
build-release:
	cargo build --release --locked

# run cargo test
test:
	cargo test

# build server container with latest tag
build-server-container:
	docker build --platform linux/amd64 -f server.Dockerfile -t $(CONTAINER_NAME):$(CONTAINER_TAG) .

# run server container with latest tag
run-server-container:
	docker run -it --rm -p 5000:5000/tcp -p 5000:5000/udp $(CONTAINER_NAME):$(CONTAINER_TAG)

# clean all targets and compiled artifacts
clean:
	cargo clean

# run kind cluster (kubernetes)
start-kind:
	kind create cluster --name gamedev --config k8s/kind_config.yaml

# deletes the kind cluster
stop-kind:
	kind delete cluster --name gamedev

# load code documentation (no deps)
doc:
	cargo doc --no-deps --open

# build and run the server with defaults
server:
	cargo build --release -p server
	PLAYER_MOVE_SPEED=150.0 CLIENT_DISCONNECT_GRACE_PERIOD=5.0 ./target/release/server

# build and run the client and connect to local server
client:
	cargo build --release -p client
	MULTIPLAYER=true ./target/release/client

# build and run the 2d client and connect to local server
client2d:
	cargo build --release -p client-2d
	MULTIPLAYER=true ./target/release/client-2d
