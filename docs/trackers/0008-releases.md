# 0008: Downloads on the Releases page

- **Status:** done in the repo; first run on GitHub pending (needs a push to `main`)
- **Started / finished:** 2026-11-09 / 2026-11-09
- **Driven by:** [feedback-log.md](../feedback-log.md), 2026-11-09 "getting new builds"

## Goal

She gets the newest NawsMapper from one bookmarked link, or from the Releases box on the
repository page, without opening Actions or unzipping twice. The release page tells a
first-time Mac user how to get past Gatekeeper, and the app shows which build it is.

## Scope

In:
- CI `release` job (after Linux checks, macOS and Windows builds) running
  `scripts/publish-release.sh`: a rolling `dev-build` pre-release on every green push to
  `main`, and a `vX.Y.Z` release when `Cargo.toml`'s version has none yet.
- Stable asset names (`NawsMapper-macOS.dmg`, `NawsMapper-Windows.exe`), so
  `releases/latest/download/…` and `releases/download/dev-build/…` are permanent links.
- macOS disk image with an Applications link instead of a zip; `codesign --verify` and
  an `hdiutil` retry in `scripts/bundle-macos.sh`.
- `scripts/release-notes.sh`: direct links, first-start help (Sequoia "Open Anyway"
  flow, older right-click flow, Windows SmartScreen), "What's new" from feat/fix/perf
  commits since the previous `v*` tag.
- Help shows `Version 0.6.0 (build 57, a1b2c3d)` for CI builds (`build_info.rs`).
- Workspace version 0.4.0 → 0.6.0 to match the roadmap (it was never bumped), which
  makes the first push publish `v0.6.0`.
- GitHub actions moved off Node 20 (checkout v7, upload-artifact v7, download-artifact v8).
- Docs: `releasing.md`, README Download section, platforms.md, user guide step 0, AGENTS.md.

Out (explicitly deferred):
- Apple notarization / Developer ID signing and Windows code signing (paid certificates).
- In-app update check.
- A styled disk image window (background picture, icon positions).
- Static CRT linking on Windows (`+crt-static`), in case a machine lacks the VC++ runtime.

## Design notes

- **Why both kinds of release.** Her habit was "newest push", and the version had not
  been bumped since 0.4 even though v0.5 and v0.6 shipped, so a release that waits for a
  manual tag would go stale again. The development build follows every push. Numbered
  releases still exist for the public and for "the version she tested", and cost one
  line in `Cargo.toml`.
- **Version bump instead of tag push.** The workflow creates the tag, so the tag and
  `Cargo.toml` can't disagree and there's one step instead of two. The `tags: ["v*"]`
  trigger was removed; tags made with `GITHUB_TOKEN` don't start workflows, so nothing loops.
- **Rolling release mechanics.** `gh release delete --cleanup-tag` then
  `gh release create --target <sha>`. Recreating (instead of re-uploading) moves the tag
  and makes the release the newest on the page. `gh release create` uploads into a draft
  and publishes at the end, so a numbered release never appears without its files.
- **Ordering.** Publish jobs share a concurrency group, and the script skips when the run's
  commit is no longer the tip of `main`, so an older run can't overwrite a newer build.
- **Permissions.** The repository's default token is read-only; only the `release` job
  gets `contents: write`.
- **Build number in the app.** `option_env!` reads `NAWSMAPPER_BUILD`/`NAWSMAPPER_COMMIT`
  at compile time. Cargo tracks those variables, so a local build without them drops
  the suffix again (checked).

## Tasks

- [x] `release` job and `scripts/publish-release.sh` (with `DRY_RUN=1`)
- [x] `scripts/release-notes.sh`
- [x] DMG in `scripts/bundle-macos.sh`
- [x] `build_info.rs` + Help menu + startup log line, unit tests
- [x] Version 0.6.0, `Cargo.lock`
- [x] Docs
- [ ] First green run on GitHub: `v0.6.0` and `dev-build` exist, both links download,
      the DMG opens with the Applications link, Help shows the build number
- [ ] Her first install from the release page (does the "Open Anyway" text match what she sees?)

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` (78 tests,
  2 new in `build_info`) on Linux.
- Release notes rendered from the real history (first release lists the two feature
  commits), and in a scratch clone with simulated tags: dev build at a tag ("None: same
  as v0.6.0"), dev and stable after feat/fix/perf/docs/chore commits (grouped, docs and
  chore left out), stable when its tag already exists, docs-only changes.
- `DRY_RUN=1 scripts/publish-release.sh` against the real repository: plans `v0.6.0`
  plus `dev-build`, and refuses when a download is missing.
- Build info: built with the variables set, the commit is in the binary; a plain rebuild
  removes it.
- Not verifiable on Linux: the macOS bundle (`hdiutil`, `codesign --verify`) and the
  Windows job. They run on the first push.

## Follow-ups

- If her Mac shows "NawsMapper is damaged" instead of the "Open Anyway" flow, the
  release notes need the `xattr` line from platforms.md.
- Repository "About" box: set the website to the latest release (command in the hand-off).
