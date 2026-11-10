#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
image=docker.io/1301182193/arceos-env@sha256:6e2cd3cd113a743967fd9f1e8cc1456cde59913a3a02e5878b9086fe696c1bcf
exec docker run --pull=missing --rm -it --platform linux/amd64 --user root --hostname arceos \
    -v "$PWD:/workspace" "$image" "$@"
