FROM ubuntu:24.04@sha256:78d4744ead1f5b19b04d0df558ab182a5300903900755e515b2ca4d10eb2044d

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl dosfstools git libclang-dev libssl-dev \
    mtools opensbi pkg-config python3 qemu-system-misc=1:8.2.2+ds-0ubuntu1.18 xxd zlib1g-dev && \
    rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup
ENV PATH="/opt/cargo/bin:/opt/riscv64-linux-musl-cross/bin:${PATH}"

# Install exactly the Rust version, components and targets requested by this checkout.
WORKDIR /opt/course
COPY arceos/rust-toolchain.toml .
RUN curl -fsSL https://sh.rustup.rs -o rustup-init.sh && \
    sh rustup-init.sh -y --profile minimal --default-toolchain none --no-modify-path && \
    rustup show active-toolchain && \
    rustup default nightly-2024-09-04 && \
    llvm="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin" && \
    ln -s "$llvm/llvm-objcopy" /usr/local/bin/rust-objcopy && \
    ln -s "$llvm/llvm-objdump" /usr/local/bin/rust-objdump

# The course CI uses this x86_64-hosted RISC-V musl compiler.
RUN curl -fL --retry 3 https://github.com/arceos-org/setup-musl/releases/download/prebuilt/riscv64-linux-musl-cross.tgz -o musl.tgz && \
    echo 'db0bc413bd4a93f2012cc74b9ba0c4af29d8bc18b88e9c61998738ccb918604b  musl.tgz' > musl.sha256 && \
    sha256sum -c musl.sha256 && tar -xzf musl.tgz -C /opt && rm musl.tgz

# Cache course dependencies, so the exercises work without a network connection.
RUN --mount=type=bind,target=/source cd /source/arceos && cargo fetch --locked && \
    chmod -R a+rwX /opt/cargo/registry /opt/cargo/git && chmod a+rwX /opt/cargo && \
    find /opt/cargo -maxdepth 1 -type f -exec chmod a+rw {} +
RUN rustup component add rust-analyzer --toolchain nightly-2024-09-04
WORKDIR /workspace
CMD ["bash"]
