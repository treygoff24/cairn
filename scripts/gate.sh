#!/bin/sh
# Canonical repo gate. Fail-fast: any failing check aborts the run.
#
# Cairn has two verification surfaces — the Rust workspace at the repo root and
# the Next.js site under site/. This script is the single command that covers
# both, so no one has to guess which surface owns a check or run an npm script
# from the wrong directory. Run it from anywhere; it cds to the repo root.
set -eu
cd "$(dirname "$0")/.."

echo "gate: rust workspace"
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

echo "gate: unused dependencies"
if command -v cargo-machete >/dev/null 2>&1; then
    # Scoped to crates/ so the scan covers exactly the workspace members. A bare
    # `cargo machete` also walks fixtures/, where a stub manifest is sample data
    # rather than a crate.
    cargo machete crates
else
    echo "gate: SKIPPED — cargo-machete not installed (cargo install cargo-machete)"
fi

echo "gate: site"
command -v pnpm >/dev/null 2>&1 || {
    echo "gate: pnpm not found; the site checks cannot run" >&2
    exit 1
}
test -d site/node_modules || {
    echo "gate: site/node_modules missing; run 'pnpm --dir site install'" >&2
    exit 1
}
pnpm --dir site lint
pnpm --dir site exec tsc --noEmit

echo "gate: all checks passed"
