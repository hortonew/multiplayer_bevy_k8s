# Multiplayer Game in Bevy on Kubernetes

The example "game" was taken from the [bevy_renet examples](https://github.com/lucaspoffo/renet/tree/master/bevy_renet/examples).

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

## Release new server version

1. Create a new release
2. Tag commit as vX.Y.Z
3. Ensure `release-container` workflow completes successfully.  (e.g. [this v0.1.0 release](https://github.com/hortonew/multiplayer_bevy_k8s/actions/runs/13473852801))
4. Pull down new version from dockerhub: `docker pull hortonew/multiplayer-bevy-server:vX.Y.Z`
5. Confirm it runs with: `docker run --platform linux/amd64 -it --rm -p 5000:5000/tcp -p 5000:5000/udp hortonew/multiplayer-bevy-server:vX.Y.Z`
