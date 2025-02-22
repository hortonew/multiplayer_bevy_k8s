# Multiplayer Game in Bevy on Kubernetes

The networking client/server example was taken from the [bevy_renet examples](https://github.com/lucaspoffo/renet/tree/master/bevy_renet/examples).  I reduced the crates needed and made it so it could run in a container.

## Dependencies
- Docker
- [Kind](https://kind.sigs.k8s.io/)

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

## Kubernetes

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
