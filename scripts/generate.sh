#!/bin/sh
# Regenerate src/Cases.mw and src/CasesPlain.mw for diff.
#
#   scripts/generate.sh
#
# The crate is built twice: with its `unicode` feature for `Cases.mw`, and
# without it for `CasesPlain.mw`, whose inline changes split lines into plain
# words.
#
# The crate version is pinned in scripts/generate/Cargo.toml. To move to a new
# one, change the pin and run this: if similar has changed, the generator stops
# and says so, and src/ has to be brought into line with it before the
# fingerprint in scripts/generate/src/main.rs is moved.
#
# Needs a Rust toolchain, and `meadow` to format and test the result.

set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/scripts/generate/Cargo.toml"

cargo run --quiet --release --manifest-path "$manifest" -- "$root"
cargo run --quiet --release --no-default-features --manifest-path "$manifest" -- "$root"

if command -v meadow >/dev/null 2>&1; then
    (cd "$root" && meadow fmt src && meadow test)
else
    echo "note: meadow is not on PATH, so the result was not formatted or tested" >&2
fi
