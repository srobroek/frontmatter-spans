# frontmatter-spans

A Rust library crate: detect YAML, TOML and JSON front matter and parse it with a source span on
every key and value. Read [docs/engineering.md](docs/engineering.md) before you change code.

- Checks: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --all-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`.
- MSRV is 1.88. Do not use newer language or library features, and check with `cargo +1.88 test`.
- No `unsafe`. Public items are documented. Hostile input returns an error and never panics.
- Commits and PR titles use Conventional Commits. Do not edit `CHANGELOG.md` or the version by
  hand: release-please owns both.
- A fix adds a regression test. Do not weaken a test to pass.
- Never push to `main`, tag, or publish. Releases go through the release PR.
