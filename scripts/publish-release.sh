#!/usr/bin/env bash
# Publishes the app downloads in dist/ as GitHub releases. CI runs this after every green
# push to main (see .github/workflows/ci.yml and docs/releasing.md):
#
#   vX.Y.Z     a normal release, created once, when the version in Cargo.toml has none yet.
#              Bumping the version and pushing is how a release is cut.
#   dev-build  a pre-release that always holds the newest build of main. It is deleted and
#              recreated on every push, so its download links never change.
#
# Usage: scripts/publish-release.sh [dist-dir]
# Needs gh and a token that may write repository contents (GH_TOKEN).
# DRY_RUN=1 prints the commands that would change anything instead of running them.
set -euo pipefail
cd "$(dirname "$0")/.."

dist=${1:-dist}
assets=("$dist/NawsMapper-macOS.dmg" "$dist/NawsMapper-Windows.exe")
for f in "${assets[@]}"; do
    [[ -f $f ]] || { echo "missing $f" >&2; exit 1; }
done

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
tag="v$version"
dev_tag=dev-build
sha=$(git rev-parse HEAD)
short=$(git rev-parse --short=7 HEAD)

run() {
    if [[ ${DRY_RUN:-} == 1 ]]; then
        printf '[dry run]'
        printf ' %q' "$@"
        echo
    else
        "$@"
    fi
}

# Runs can finish out of order. A newer commit publishes itself in its own run;
# never replace its downloads with older ones.
if [[ -n ${GITHUB_ACTIONS:-} ]]; then
    head=$(gh api "repos/$GITHUB_REPOSITORY/commits/$GITHUB_REF_NAME" --jq .sha)
    if [[ $head != "$sha" ]]; then
        echo "$GITHUB_REF_NAME has moved on to $head, whose run publishes. Nothing to do."
        exit 0
    fi
fi

notes=$(mktemp)
trap 'rm -f "$notes"' EXIT

if gh release view "$tag" >/dev/null 2>&1; then
    echo "Release $tag exists. Bump the version in Cargo.toml to cut the next one."
else
    echo "Creating release $tag from $short"
    bash scripts/release-notes.sh stable "$tag" >"$notes"
    run gh release create "$tag" "${assets[@]}" --target "$sha" \
        --title "NawsMapper $version" --notes-file "$notes"
    # So the dev build's notes count their changes from this new release.
    run git fetch --quiet --force origin "refs/tags/$tag:refs/tags/$tag"
fi

echo "Replacing $dev_tag with $short"
bash scripts/release-notes.sh dev "$dev_tag" >"$notes"
if gh release view "$dev_tag" >/dev/null 2>&1; then
    run gh release delete "$dev_tag" --cleanup-tag --yes
fi
run gh release create "$dev_tag" "${assets[@]}" --target "$sha" --prerelease --latest=false \
    --title "Development build $(git log -1 --format=%cs HEAD) ($short)" --notes-file "$notes"
