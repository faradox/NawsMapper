#!/usr/bin/env bash
# Bumps the workspace version and commits it. Pushing that commit to main makes
# CI publish the numbered release (see docs/releasing.md); this script neither
# pushes nor tags.
#
# Usage: scripts/bump-version.sh [patch|minor|major]   (default: patch)
#        make bump [patch|minor|major]
#
# Changes, then commits only these files:
#   Cargo.toml   [workspace.package] version
#   Cargo.lock   the workspace crates' versions
#   README.md    the "Status: **vX.Y.**" line, if present
set -euo pipefail
cd "$(dirname "$0")/.."

part=${1:-patch}
case $part in
    patch | minor | major) ;;
    *)
        echo "usage: make bump [patch|minor|major]   (got '$part')" >&2
        exit 2
        ;;
esac

files=(Cargo.toml Cargo.lock README.md)
if ! git diff --quiet HEAD -- "${files[@]}"; then
    echo "Commit or stash the changes to ${files[*]} first; the bump commits these files." >&2
    git status --short -- "${files[@]}" >&2
    exit 1
fi

current=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
if ! [[ $current =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    echo "Cargo.toml version '$current' is not MAJOR.MINOR.PATCH" >&2
    exit 1
fi
major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]} patch=${BASH_REMATCH[3]}
case $part in
    major) next="$((major + 1)).0.0" ;;
    minor) next="$major.$((minor + 1)).0" ;;
    patch) next="$major.$minor.$((patch + 1))" ;;
esac
short=${next%.*}

# The first `version = "…"` line is [workspace.package]'s. Perl, not `sed -i`,
# so this behaves the same on macOS and Linux.
NEXT=$next perl -pi -e 's/^version = "[^"]*"/version = "$ENV{NEXT}"/ if !$done && /^version = / && ($done = 1)' Cargo.toml
# Only the workspace crates' entries change; no network needed.
cargo update --workspace --offline --quiet
SHORT=$short perl -pi -e 's/^(> Status: \*\*v)[0-9]+\.[0-9]+(\.\*\*)/$1$ENV{SHORT}$2/' README.md

git commit --quiet -F - -- "${files[@]}" <<EOF
chore(release): bump version to $next

- Bump \`version\` in \`Cargo.toml\` from $current to $next and update \`Cargo.lock\`
- Update the README status line to v$short
- Run \`cargo update --workspace --offline\`
EOF

echo "Bumped $current -> $next and committed. Publish it with:"
echo "  git push"
echo "When CI is green, v$next is on the Releases page."
