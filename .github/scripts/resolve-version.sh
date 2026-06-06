#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
#
# Resolve and normalise the release version for the packaging jobs of
# .github/workflows/release.yml. Centralised so the build, snap and flatpak
# jobs share one implementation instead of three drifting copies.
#
# Usage: resolve-version.sh <event_name> <ref_name> <input_version>
#   <event_name>     github.event_name  (e.g. "push" or "workflow_dispatch")
#   <ref_name>       github.ref_name    (the tag name on a tag push)
#   <input_version>  github.event.inputs.version (the typed value on dispatch)
#
# Prints a Cargo-valid semver (major.minor.patch[-pre][+build]) on stdout, or
# exits non-zero with a GitHub `::error::` annotation for an unusable value.
set -euo pipefail

event_name="${1:-}"
ref_name="${2:-}"
input="${3:-}"

# A manual run uses the typed input; a tag push uses the tag name. Keyed on
# event_name (not ref_type) so a workflow_dispatch launched from a tag ref still
# honours the input.
if [ "$event_name" = "workflow_dispatch" ]; then
  v="$input"
else
  v="$ref_name"
fi
v="${v:-2.17.0}"   # fallback mirrors GLPI's version line (see UPSTREAM.md)
v="${v#v}"         # tolerate a leading "v" (v2.17.0 -> 2.17.0) from either source

# Split optional -prerelease / +build metadata from the numeric core.
core="${v%%[-+]*}"
suffix="${v#"$core"}"

# The core must be 1-3 dot-separated numbers; the suffix (if any) only the
# semver-allowed set. Anything else (a slash-bearing ref, stray text) is fatal
# here — cheaply, before any build — rather than breaking the later sed stamp.
if ! printf '%s' "$core" | grep -Eq '^[0-9]+(\.[0-9]+){0,2}$'; then
  echo "::error::Invalid release version '$v' (expected e.g. 1.7.0)." >&2
  exit 1
fi
if [ -n "$suffix" ] && ! printf '%s' "$suffix" | grep -Eq '^[-+][0-9A-Za-z.-]+$'; then
  echo "::error::Invalid release version '$v' (bad pre-release/build metadata)." >&2
  exit 1
fi

# Pad the core to major.minor.patch — Cargo requires all three components, so a
# typed "1.7" must become "1.7.0".
case "$core" in
  *.*.*) : ;;
  *.*)   core="$core.0" ;;
  *)     core="$core.0.0" ;;
esac

printf '%s\n' "$core$suffix"
