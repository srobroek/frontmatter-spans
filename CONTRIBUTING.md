# Contributing

Thanks for helping with `frontmatter-spans`. Engineering standards, the lint set and the
release flow are in [docs/engineering.md](docs/engineering.md).

## Setup

You need [rustup](https://rustup.rs). `rust-toolchain.toml` pins the stable toolchain with
rustfmt and clippy, and rustup installs it on first use. The minimum supported Rust version
is 1.88.

```sh
git clone https://github.com/srobroek/frontmatter-spans
cd frontmatter-spans
cargo test --all-features
```

## Checks

CI runs these on every pull request. Run them before you push:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo deny check     # needs cargo-deny
```

Library code in `src/lib.rs` also warns on clippy restriction lints such as `unwrap_used`,
`indexing_slicing` and `as_conversions`, and `-D warnings` turns those warnings into CI failures.
`clippy.toml` lets test code unwrap, expect, panic, index and print. Write library code to return
errors instead, and keep it compatible with Rust 1.88.

Fuzzing needs a nightly toolchain and `cargo install cargo-fuzz --locked`:

```sh
cargo +nightly fuzz run parse -- -max_total_time=60
```

A fix comes with a regression test in `tests/regressions.rs`. A parser change comes with a
property or behaviour test.

## Commits and pull requests

Use [Conventional Commits](https://www.conventionalcommits.org/), for example
`feat: parse TOML dotted keys` or `fix: report the key span for duplicate YAML keys`.
Release-please reads them to choose the next version and write the changelog. The pull request
title is checked against the same format, and maintainers squash-merge, so the title becomes
the commit message.

Open a pull request with a short description and the checks you ran.

## Licence

Contributions are licensed under Apache-2.0, the licence of the project.

## Releases

Maintainers only. Release-please opens a release pull request from the Conventional Commits on
`main`. Merging it tags the release and publishes to crates.io through trusted publishing. See
[docs/engineering.md](docs/engineering.md#release-flow).
