#!/usr/bin/env bash
# Prints the Markdown body of a GitHub release: direct download links, first-start help
# for an app that isn't notarized, and the user-facing changes (feat/fix/perf commits).
#
# Usage: scripts/release-notes.sh <stable|dev> <release-tag>
#   stable  lists the changes since the previous v* tag
#   dev     lists the changes since the newest v* tag (the build is ahead of it)
# The release tag only goes into the download links; the commit is always HEAD.
set -euo pipefail
cd "$(dirname "$0")/.."

kind=${1:?usage: release-notes.sh <stable|dev> <release-tag>}
tag=${2:?usage: release-notes.sh <stable|dev> <release-tag>}
[[ $kind == stable || $kind == dev ]] || { echo "kind must be stable or dev, not '$kind'" >&2; exit 2; }

repo=${GITHUB_REPOSITORY:-$(git remote get-url origin | sed -E 's#^(git@github\.com:|https://github\.com/)##; s#\.git$##')}
url="https://github.com/$repo"
sha=$(git rev-parse HEAD)
short=$(git rev-parse --short=7 HEAD)
date=$(git log -1 --format=%cs HEAD)
dl="$url/releases/download/$tag"

if [[ $kind == stable ]]; then
    since=$(git describe --tags --abbrev=0 --match 'v[0-9]*' --exclude "$tag" HEAD 2>/dev/null || true)
else
    since=$(git describe --tags --abbrev=0 --match 'v[0-9]*' HEAD 2>/dev/null || true)
fi
range=HEAD
[[ -n $since ]] && range="$since..HEAD"

# Bullets for one conventional-commit type: "feat(render): add x" -> "- Add x".
changes() {
    git log --no-merges --format=%s "$range" |
        sed -nE "s/^$1(\([^)]*\))?!?: +//p" |
        awk '{ print "- " toupper(substr($0, 1, 1)) substr($0, 2) }'
}

if [[ $kind == dev ]]; then
    echo "The newest **development build**, made automatically from commit" \
        "[\`$short\`]($url/commit/$sha) on $date. It has changes that aren't in a numbered" \
        "release yet, so expect the odd rough edge. The stable version is on the" \
        "[latest release]($url/releases/latest) page."
    echo
fi

cat <<EOF
## Download

- **Mac** (Apple Silicon and Intel, macOS 11 or newer): [NawsMapper-macOS.dmg]($dl/NawsMapper-macOS.dmg)
- **Windows** (64-bit, Windows 10 or newer): [NawsMapper-Windows.exe]($dl/NawsMapper-Windows.exe)

You don't need the "Source code" files in the list at the bottom.

### First start on a Mac

NawsMapper is free and not registered with Apple, so macOS asks for your permission the
first time you open each new version:

1. Open \`NawsMapper-macOS.dmg\` and drag **NawsMapper** onto **Applications**
   (choose **Replace** if an older version is there).
2. Open NawsMapper from Applications. macOS says it can't verify the app: click **Done**.
3. Open **System Settings → Privacy & Security**, scroll down to the message about
   NawsMapper and click **Open Anyway**. Confirm with your password or Touch ID.

On macOS 14 Sonoma or older: right-click NawsMapper in Applications, choose **Open**, then **Open** again.

### First start on Windows

If Windows says "Windows protected your PC", click **More info**, then **Run anyway**.

EOF

feat=$(changes feat)
fix=$(changes fix)
perf=$(changes perf)

if [[ $kind == stable ]]; then
    echo "## What's new"
elif [[ -n $since ]]; then
    echo "## Changes since $since"
else
    echo "## Changes"
fi
echo
if [[ -z $feat$fix$perf ]]; then
    if [[ $kind == dev && -n $since && $(git rev-parse "$since^{commit}") == "$sha" ]]; then
        echo "None: this build is the same as the $since release."
    else
        echo "Small improvements behind the scenes."
    fi
    echo
fi
[[ -n $feat ]] && printf '### New\n\n%s\n\n' "$feat"
[[ -n $fix ]] && printf '### Fixes\n\n%s\n\n' "$fix"
[[ -n $perf ]] && printf '### Faster\n\n%s\n\n' "$perf"

if [[ -n $since ]]; then
    echo "[Every change since $since]($url/compare/$since...$sha)"
else
    echo "[Every change]($url/commits/$sha)"
fi
