#!/usr/bin/env bash
# Fail closed unless the release tag exists on the remote, resolves to the
# release commit, and names the version in Cargo.toml.
# Inputs (environment): TAG, SHA, REPO (owner/name).
set -euo pipefail

: "${TAG:?}" "${SHA:?}" "${REPO:?}"
case "$TAG" in v[0-9]*) ;; *) echo "Unexpected tag name: ${TAG}" >&2; exit 1 ;; esac
case "$SHA" in *[!0-9a-f]* | "") echo "Unexpected commit id: ${SHA}" >&2; exit 1 ;; esac

refs=$(git ls-remote "https://github.com/${REPO}.git" "refs/tags/${TAG}" "refs/tags/${TAG}^{}")
# An annotated tag lists its target on the peeled line; a lightweight tag lists it directly.
peeled=$(awk -v r="refs/tags/${TAG}^{}" '$2 == r { print $1 }' <<<"$refs")
direct=$(awk -v r="refs/tags/${TAG}" '$2 == r { print $1 }' <<<"$refs")
target="${peeled:-$direct}"
if [ -z "$target" ]; then
  echo "Tag ${TAG} not found on the remote." >&2
  exit 1
fi
if [ "$target" != "$SHA" ]; then
  echo "Tag ${TAG} points at ${target}, not the release commit ${SHA}." >&2
  exit 1
fi

version=$(cargo metadata --no-deps --format-version 1 --locked |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
if [ "$version" != "${TAG#v}" ]; then
  echo "Cargo.toml version ${version} does not match tag ${TAG}." >&2
  exit 1
fi
echo "Tag ${TAG} -> ${SHA}, version ${version}."
