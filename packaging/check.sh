#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
if [ "${GITHUB_ACTIONS:-}" = true ]; then
    dbus-run-session -- cargo test --locked place::tests::placement_callback_runs_without_an_application_runtime -- --exact --ignored
fi
find packaging -name '*.sh' -exec sh -n {} \;
sh -n packaging/appimage/AppRun
