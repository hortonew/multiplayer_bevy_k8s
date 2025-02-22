# Example: Bevy Multiplayer Game on Kubernetes

## Local

```sh
cargo run -p server
cargo run -p client
```

## Local docker

```sh
# server
docker build -f server.Dockerfile -t multiplayer-bevy-server:latest .
docker run -it --rm -p 5000:5000/tcp -p 5000:5000/udp multiplayer-bevy-server:latest

# client
cargo run -p client
```

## Remote
