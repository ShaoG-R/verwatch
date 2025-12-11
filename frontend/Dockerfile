FROM archlinux:latest AS builder

# Update system and install dependencies
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
    rustup \
    binaryen \
    gcc \
    pkg-config \
    openssl \
    && pacman -Scc --noconfirm

# Setup Rust
RUN rustup default stable && \
    rustup target add wasm32-unknown-unknown

# Install trunk and wasm-bindgen-cli
RUN cargo install trunk wasm-bindgen-cli --locked

# Set working directory
WORKDIR /app

# Copy source files (from repo root context)
COPY shared ./shared
COPY frontend ./frontend

# Build
WORKDIR /app/frontend
RUN trunk build --release

# Export stage - only output the dist directory
FROM scratch AS export
COPY --from=builder /app/frontend/dist /dist
