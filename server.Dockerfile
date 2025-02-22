# Builder stage
FROM rust:latest AS builder
WORKDIR /app
# Install dependencies required for alsa-sys
RUN apt-get update && apt-get install -y \
    pkg-config \
    libasound2-dev \
    libudev-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

COPY server/Cargo.toml .
COPY server/src/ ./src/
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y \
    mesa-vulkan-drivers \
    libegl1 \
    libgles2 \
    vulkan-tools \
    libasound2-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

ENV WGPU_BACKEND="vulkan"
ENV MESA_LOADER_DRIVER_OVERRIDE="lavapipe"
ENV VK_ICD_FILENAMES="/usr/share/vulkan/icd.d/lvp_icd.*.json"
ENV VK_LAYER_PATH="/usr/share/vulkan/implicit_layer.d"
EXPOSE 5000
COPY --from=builder /app/target/release/server /app/
RUN chmod +x /app/server
CMD ["sh", "-c", "WINIT_UNIX_BACKEND=headless /app/server"]
