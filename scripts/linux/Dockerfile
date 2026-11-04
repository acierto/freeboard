# Headless Linux verification image (Ubuntu 22.04, the CI baseline).
# Builds and tests Freeboard on Linux without a desktop session.
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential curl ca-certificates git pkg-config file xz-utils \
      libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev patchelf \
      xvfb dbus-x11 \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && apt-get install -y nodejs && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH=/root/.cargo/bin:$PATH
RUN git config --global user.name "Freeboard Linux" && git config --global user.email "linux@freeboard.invalid" \
    && git config --global init.defaultBranch main
WORKDIR /work
