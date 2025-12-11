FROM archlinux:latest AS builder

# Update system and install dependencies
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
    rustup \
    binaryen \
    gcc \
    pkg-config \
    openssl \
    curl \
    && pacman -Scc --noconfirm

# Setup Rust
RUN rustup default stable && \
    rustup target add wasm32-unknown-unknown

# Add cargo bin to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Install trunk
RUN cargo install trunk --locked

# Set working directory
WORKDIR /app

# Copy source files (from repo root context)
COPY shared ./shared
COPY frontend ./frontend

# Download latest CSS/JS assets (replace any existing)
WORKDIR /app/frontend
RUN rm -rf assets && mkdir -p assets && \
    curl -o assets/daisyui.css https://cdn.jsdelivr.net/npm/daisyui@5/daisyui.css && \
    curl -o assets/tailwind.js https://unpkg.com/@tailwindcss/browser@4

# Build
RUN trunk build --release

# Export stage - only output the dist directory
FROM scratch AS export
COPY --from=builder /app/frontend/dist /dist
