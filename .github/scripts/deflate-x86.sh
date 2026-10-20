#!/usr/bin/env bash
set -euo pipefail
test "$RUNNER_ARCH" = X64
mkdir -p results photos fixtures
{
  uname -a
  lscpu
  rustc -Vv
  cargo -V
  git -C candidate rev-parse --short HEAD
  git -C baseline rev-parse --short HEAD
  env | sort | grep -E '^(CARGO_PROFILE_|RUSTC_WRAPPER=|RUSTC_WORKSPACE_WRAPPER=|RAYON_NUM_THREADS=|RUSTFLAGS=)' || true
} > results/host.txt

# Both builds use the same worker and profile, in separate target directories.
cp candidate/crates/io/examples/bench_deflate.rs baseline/crates/io/examples/bench_deflate.rs
nice -n 10 cargo test --locked --manifest-path candidate/Cargo.toml -p photocraft-io --example bench_deflate
for version in baseline candidate; do
  cargo tree --locked --manifest-path "$version/Cargo.toml" -e features -i flate2 > "results/$version-features.txt"
  nice -n 10 cargo build --locked --release --manifest-path "$version/Cargo.toml" -p photocraft-io --example bench_deflate
  sha256sum "$version/target/release/examples/bench_deflate" >> results/binaries.sha256
done
curl -fsSL --retry 3 -A Photocraft-dev -o photos/IMG_4059.CR2 'https://raw.pixls.us/data/Canon/PowerShot%20SX50%20HS/IMG_4059.CR2'
curl -fsSL --retry 3 -A Photocraft-dev -o photos/P1000475.RW2 'https://raw.pixls.us/data/Panasonic/DC-G9/P1000475.RW2'
grep -E '  (IMG_4059.CR2|P1000475.RW2)$' candidate/xtask/pixls.sha256 > results/photos.sha256
(cd photos && sha256sum --check ../results/photos.sha256)
nice -n 10 baseline/target/release/examples/bench_deflate prepare photos fixtures > results/fixtures.jsonl

# One warmup and nine alternating pairs. No builds or downloads overlap measurements.
for trial in $(seq 0 9); do
  versions='baseline candidate'
  if (( trial % 2 )); then versions='candidate baseline'; fi
  for version in $versions; do
    "$version/target/release/examples/bench_deflate" measure fixtures "$version" "$trial" >> results/samples.jsonl
  done
done
candidate/target/release/examples/bench_deflate summarize results/samples.jsonl > results/summary.md
