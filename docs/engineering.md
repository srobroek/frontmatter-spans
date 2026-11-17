# Engineering

Standards for `frontmatter-spans`. [CONTRIBUTING.md](../CONTRIBUTING.md) covers how to submit a change.

## Standards

- Edition 2024, library only, no `unsafe` (`unsafe_code = "forbid"`).
- Errors use `thiserror`. The library never prints and never panics on input: hostile input
  returns a `ParseError` with a span.
- Public items are documented and the crate root has crate-level docs (the README is the
  crate documentation). Public error and contract types stay stable under semver.
- Dependencies are few and justified: `saphyr`/`saphyr-parser` (YAML events), `toml_edit`
  (TOML spans), `serde`, `thiserror`. JSON is parsed inside the crate because common JSON crates
  drop spans. `cargo deny` enforces licences (permissive only), sources (crates.io only) and warns
  on duplicate versions.

## Lints

Configured in `Cargo.toml` and enforced in CI with `-D warnings`.

- rustc: `unsafe_code = "forbid"`, `missing_docs`, `unreachable_pub`, `rust_2018_idioms`.
- rustdoc, all denied: `broken_intra_doc_links`, `private_intra_doc_links`, `invalid_html_tags`,
  `bare_urls`, `missing_crate_level_docs`.
- clippy: `pedantic`, with `module_name_repetitions`, `missing_errors_doc` and
  `missing_panics_doc` allowed.

An `#[allow]` or `#[expect]` needs a `reason`.

## MSRV policy

The minimum supported Rust version is **1.88**, set as `rust-version` in `Cargo.toml` and tested
by the `msrv` CI job with `cargo +1.88 test --all-features`. 1.88 is the floor because the code
uses let chains (stable in edition 2024 from 1.88) and the dev-dependency `proptest` 1.12 needs
1.88. Raising the MSRV is a minor-version change while the crate is 0.x and is called out in the
changelog. Update `rust-version`, the `msrv` job name and command, the README and
CONTRIBUTING.md together.

`rust-toolchain.toml` pins the stable toolchain used for development and for the lint, test and
release jobs. Bump it with a normal change.

## Testing

- `tests/behaviour.rs`, `spans.rs`, `yaml_subset.rs`, `limits.rs`: example-based tests through
  the public API.
- `tests/properties.rs`, `tests/generated_yaml.rs`: proptest properties. Spans lie inside the
  document on character boundaries, and written keys and values sit at their spans.
- `tests/regressions.rs`: one test per fixed bug. A fix lands with its regression test.
- `tests/*.proptest-regressions`: proptest failure seeds. Commit them.
- `fuzz/`: a self-contained cargo-fuzz crate (target `parse`), outside the main package. It needs
  nightly and `cargo install cargo-fuzz --locked`. The `Fuzz` workflow runs it weekly for 60
  seconds, and you can start it by hand from the Actions tab.

```sh
cargo +nightly fuzz run parse -- -max_total_time=60
```

Never weaken an assertion or edit an expected value just to pass.

## CI

`ci.yml` runs on pull requests and on pushes to `main`: `lint` (fmt, clippy, tests, docs),
`test` (macOS and Windows), `msrv`, `package` (`cargo publish --dry-run`), `semver`
(cargo-semver-checks, pull requests only, skipped until a release is on crates.io), `deny`,
`pr-title` (pull requests only) and `ci-ok`. `ci-ok` aggregates the rest and is the one required
status check. Every action is pinned by commit SHA with a version comment, and Dependabot
updates them weekly.

## Release flow

Releases use [release-please](https://github.com/googleapis/release-please) with Conventional
Commits. Version 0.x uses `bump-minor-pre-major`, so a breaking change bumps the minor version.

1. Merge changes to `main` with Conventional Commit titles.
2. Release-please keeps a release pull request open with the version bump (`Cargo.toml`,
   `Cargo.lock`) and the changelog.
3. Merge the release pull request. Release-please tags `vX.Y.Z` and creates the GitHub release.
4. The `publish` job runs in the `release` environment: it checks out the tag, runs the tests,
   skips if crates.io already has the version, then exchanges its OIDC token for a short-lived
   crates.io token ([trusted publishing](https://crates.io/docs/trusted-publishing)) and runs
   `cargo publish`.

### One-time setup and the first publish

Trusted publishing needs the crate to exist, so the first release is published by hand.

1. Create the repository and install the release app (variable `RELEASE_APP_CLIENT_ID`, secret
   `RELEASE_APP_PRIVATE_KEY`). Create the `release` environment.
2. Publish 0.1.0 by hand from a clean checkout of `main` after this setup merges: `cargo publish`.
   Then push an annotated `v0.1.0` tag at that exact commit. release-please reads the tag as the
   last release, so the next release pull request counts only commits made after it.
3. On crates.io, add a trusted publisher for this repository: workflow `release.yml`,
   environment `release`.
4. Merge changes as usual. The manifest starts at `0.1.0` and the `v0.1.0` tag marks it, so the
   first release pull request bumps from there (for example to `0.2.0` after a `feat`). If a release
   ever targets a version that crates.io already has, the `publish` job detects it and skips
   instead of failing.
