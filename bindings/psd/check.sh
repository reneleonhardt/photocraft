#!/bin/sh
set -eu
cd "$(dirname "$0")"
PSD_BINDINGS_PYTHON="${PSD_BINDINGS_PYTHON:-python3}"
checks="${1:-all}"
case "$checks" in all|c|javascript) ;; *) echo 'usage: check.sh [all|c|javascript]' >&2; exit 1 ;; esac

if [ "$checks" != javascript ]; then
    cargo fmt --all -- --check
    cargo test --all-features --locked
    cargo clippy --all-features --all-targets --locked -- -D warnings
fi
if [ "$checks" != c ]; then
    export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-} -C target-feature=+simd128"
    cargo clippy --target wasm32-unknown-unknown --features javascript --lib --locked -- -D warnings
fi

cargo run --example fixture --locked -- target/fixture.psd
cargo run --example fixture --locked -- --matrix target/fixtures
# Nonzero resource-name padding is accepted and normalized by the writer.
"$PSD_BINDINGS_PYTHON" - <<'PY'
from pathlib import Path
data = bytearray(Path("target/fixture.psd").read_bytes())
name = 26 + 4 + int.from_bytes(data[26:30], "big") + 4 + 6
assert data[name] == 0
data[name + 1] = 1
Path("target/normalized-input.psd").write_bytes(data)
PY
if [ "$checks" != javascript ]; then
    sh package-c.sh
    sh check-sdk.sh
fi

if [ "$checks" = all ]; then
    "$PSD_BINDINGS_PYTHON" -m maturin build --release --strip --features python --locked --interpreter "$PSD_BINDINGS_PYTHON" --out target/wheels-release
    "$PSD_BINDINGS_PYTHON" -m pip install --no-index --find-links target/wheels-release --no-deps --force-reinstall photocraft-psd
    "$PSD_BINDINGS_PYTHON" tests/python.py target/fixture.psd
    "$PSD_BINDINGS_PYTHON" examples/inspect.py target/normalized-input.psd target/python.psd
    cmp target/fixture.psd target/python.psd
    "$PSD_BINDINGS_PYTHON" examples/create.py target/created-python.psd
    cmp target/created-c.psd target/created-python.psd
fi

if [ "$checks" != c ]; then
    export WASM_PACK_CACHE="${WASM_PACK_CACHE:-$PWD/target/wasm-pack-cache}"
    command -v wasm-opt >/dev/null || { echo 'Install the latest stable Binaryen (wasm-opt) and add it to PATH.' >&2; exit 1; }
    wasm-pack build --release --target nodejs --out-dir pkg-node --out-name photocraft_psd -- --features javascript --locked
    node tests/javascript.mjs target/fixture.psd
    node examples/inspect.mjs target/normalized-input.psd target/javascript.psd
    cmp target/fixture.psd target/javascript.psd
    node examples/create.mjs target/created-javascript.psd
    wasm-pack build --release --target web --out-dir pkg --out-name photocraft_psd -- --features javascript --locked
    node tests/web.mjs target/fixture.psd
    npm ci --ignore-scripts
    npm audit --audit-level=high
    npm run check
    npm run check:worker
fi
