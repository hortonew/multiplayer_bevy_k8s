# Multiplayer Game in Bevy on Kubernetes

[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Docker][docker-badge]][docker-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/hortonew/multiplayer_bevy_k8s/blob/main/LICENSE
[actions-badge]: https://github.com/hortonew/multiplayer_bevy_k8s/actions/workflows/release-container.yml/badge.svg
[actions-url]: https://github.com/hortonew/multiplayer_bevy_k8s/actions
[docker-badge]: https://img.shields.io/badge/dockerhub-images-important.svg?logo=Docker&color=blue
[docker-url]: https://hub.docker.com/repository/docker/hortonew/multiplayer-bevy-server/general

The networking client/server example was taken from the [bevy_renet examples](https://github.com/lucaspoffo/renet/tree/master/bevy_renet/examples).  I reduced the crates needed and made it so it could run in a container.

Warning: This is not production ready, but I hope to keep adding examples on how to scale out the game servers, maintain state, reduce reconnects, etc.

3d Example
![Example](/images/example.gif)

2d Example
![Example 2d](/images/example-2d.gif)

## Run it

### Local

```sh
just server # one window
just client # another window
```

### Local docker

```sh
# Build
docker build -f server.Dockerfile -t multiplayer-bevy-server:latest . # build it yourself
docker pull hortonew/multiplayer-bevy-server:latest # or get from Dockerhub

# Run
docker run -it --rm -p 5000:5000/tcp -p 5000:5000/udp multiplayer-bevy-server:latest
cargo run -p client

# Use a different server/port
export SERVER_IP=a.b.c.d
export SERVER_PORT=5000
export MULTIPLAYER=true

cargo build --release -p server
./target/release/server

cargo build --release -p client
./target/release/client
```

### Kubernetes

```sh
# Apply the metrics server
kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml

# Apply MetalLB load balancer and an IP Address pool
kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/main/config/manifests/metallb-native.yaml
kubectl apply -f k8s/manifests/loadbalancer-pool.yaml

# Apply the gamedev namespace, statefulset, service, and pod autoscaler
kubectl apply -f k8s/manifests/multiplayer-game-service.yaml
```

## Release new server version

1. Create a new release
2. Tag commit as vX.Y.Z
3. Ensure `release-container` workflow completes successfully.  (e.g. [this v0.1.0 release](https://github.com/hortonew/multiplayer_bevy_k8s/actions/runs/13473852801))
4. Pull down new version from dockerhub: `docker pull hortonew/multiplayer-bevy-server:vX.Y.Z`
5. Confirm it runs with: `docker run --platform linux/amd64 -it --rm -p 5000:5000/tcp -p 5000:5000/udp hortonew/multiplayer-bevy-server:vX.Y.Z`

## Next Steps

- Hand off client state to new pods during reconnects (maintain state elsewhere).  Thoughts:  sidecar container with API that can get/put state.  State could be stored in Redis, or similar services.
- Terraform EKS build example
- Work on a more realistic game example
- CI/CD: Build release artifacts for Mac, Windows, Linux, Mobile
