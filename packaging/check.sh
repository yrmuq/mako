#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
find packaging -name '*.sh' -exec sh -n {} \;
sh -n packaging/appimage/AppRun
