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
    nodejs \
    npm \
    && pacman -Scc --noconfirm

# Setup Rust
RUN rustup default stable && \
    rustup target add wasm32-unknown-unknown

# Add cargo bin to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Install trunk (specific version) and wasm-bindgen-cli (matching version trunk expects)
RUN cargo install trunk@0.21.14 --locked && \
    cargo install wasm-bindgen-cli@0.2.100 --locked

# Set working directory
WORKDIR /app

# Copy source files (from repo root context)
COPY shared ./shared
COPY frontend ./frontend

# Build optimized CSS with Tailwind (scans .rs files, only includes used classes)
WORKDIR /app/frontend
RUN npm install && npm run build:css

# Build WASM
RUN trunk build --release --frozen

# Export stage - only output the dist directory
FROM scratch AS export
COPY --from=builder /app/frontend/dist /dist
