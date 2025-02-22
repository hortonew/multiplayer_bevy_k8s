# Example: Bevy Multiplayer Game on Kubernetes

## Local

```sh
cargo run -p server
cargo run -p client
```

## Local docker

```sh
# server
docker build -f server.Dockerfile -t multiplayer_server .
docker run -it --rm -p 5000:5000/tcp -p 5000:5000/udp multiplayer_server

# client
cargo run -p client
```

## Remote
