# Contributing to orecchiette-sdr-pluto-rs

First off, thank you for considering contributing! This crate
implements the `SdrSource` trait (from `orecchiette-sdr-source-rs`)
for the Analog Devices ADALM-Pluto via the `industrial-io` crate,
which wraps the `libiio` C library.

## Quick Start

```bash
git clone https://github.com/isaacbentley/orecchiette-sdr-pluto-rs.git
cd orecchiette-sdr-pluto-rs

# Requires libiio-dev (Ubuntu/Debian) or a source build of libiio
# (macOS) — see the README's Installation section.
cargo test
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check
```

## Testing Hardware Changes

Most of this crate's logic (gain configuration, LO retuning, dwell
pacing) can be unit-tested without a device attached. If your
change affects the actual capture loop, please test against real
PlutoSDR hardware (USB or network) before opening a PR.

## Code Style

We use standard `rustfmt` defaults. Please run `cargo fmt --all` before pushing.

Clippy is run with `-D warnings` in CI. If a lint is genuinely wrong for the situation, allow it with a `// ALLOW:` justification comment explaining why.

## Pull Requests

- **Commit messages:** Describe *why* the change is needed and *what* it changes.
- **Templates:** Please fill out the Pull Request template when opening a PR.

## License

By contributing, you agree your contributions will be licensed under GPL-3.0-or-later, the same as the rest of the project.
