# Releases and downloads

People download NawsMapper from the
[Releases page](https://github.com/faradox/NawsMapper/releases), never from the
Actions tab. CI keeps two kinds of release there, both built by the `release` job in
[.github/workflows/ci.yml](../.github/workflows/ci.yml) after every push to `main` that
passes all jobs. The work is done by [scripts/publish-release.sh](../scripts/publish-release.sh).

| Release | Tag | When it changes | Shown as |
| --- | --- | --- | --- |
| Numbered release | `vX.Y.Z` | Once, the first green push after the version in `Cargo.toml` changes | **Latest** in the sidebar |
| Development build | `dev-build` | Every green push to `main` (replaced, not added) | Pre-release |

## Permanent download links

These URLs never change. Bookmark them, put them in the README, send them to testers.

| | Newest numbered release | Newest development build |
| --- | --- | --- |
| Mac | [releases/latest/download/NawsMapper-macOS.dmg](https://github.com/faradox/NawsMapper/releases/latest/download/NawsMapper-macOS.dmg) | [releases/download/dev-build/NawsMapper-macOS.dmg](https://github.com/faradox/NawsMapper/releases/download/dev-build/NawsMapper-macOS.dmg) |
| Windows | [releases/latest/download/NawsMapper-Windows.exe](https://github.com/faradox/NawsMapper/releases/latest/download/NawsMapper-Windows.exe) | [releases/download/dev-build/NawsMapper-Windows.exe](https://github.com/faradox/NawsMapper/releases/download/dev-build/NawsMapper-Windows.exe) |
| Release page | [releases/latest](https://github.com/faradox/NawsMapper/releases/latest) | [releases/tag/dev-build](https://github.com/faradox/NawsMapper/releases/tag/dev-build) |

They depend on the file names `NawsMapper-macOS.dmg` and `NawsMapper-Windows.exe`. Don't
rename them.

## Cutting a numbered release

```sh
make bump            # 0.7.0 -> 0.7.1 (patch, the default)
make bump minor      # 0.7.0 -> 0.8.0
make bump major      # 0.7.0 -> 1.0.0
git push
```

`make bump` ([scripts/bump-version.sh](../scripts/bump-version.sh)) bumps `version` under
`[workspace.package]` in `Cargo.toml`, updates the workspace crates in `Cargo.lock`
(offline) and the README status line, and commits exactly those three files as
`chore(release): bump version to X.Y.Z`. It stops if they have other uncommitted
changes, and it neither pushes nor tags. Update `docs/roadmap.md` by hand when a
milestone is done.

When CI is green, `v0.7.0` appears on the Releases page with the downloads, first-start
instructions and a "What's new" list. There's no tag to push by hand. The workflow
creates the tag itself, and pushing a `v*` tag yourself does nothing.

"What's new" is built from the `feat`, `fix` and `perf` commit subjects since the previous
`v*` tag ([scripts/release-notes.sh](../scripts/release-notes.sh)), so write those
subjects for a reader who uses the app. To reword the notes afterwards, edit the release
on GitHub. CI never touches a numbered release again once it exists.

## Which build is this?

CI compiles the workflow run number and commit into the app (`NAWSMAPPER_BUILD`,
`NAWSMAPPER_COMMIT`, read by `crates/om-app/src/build_info.rs`). **Help** shows
`Version 0.6.0 (build 57, a1b2c3d)`. Local builds show just `Version 0.6.0`. Ask testers to
include that line in feedback; it goes into the heading of the
[feedback-log.md](feedback-log.md) entry.

## When something goes wrong

- **The "Publish downloads" job failed.** Re-run it from the Actions tab. It's safe to
  repeat: an existing numbered release is left alone and the development build is
  recreated. If it failed after deleting the development build, the dev links return
  404 until the job succeeds.
- **A release is missing a file or was made from the wrong commit.** Delete the release
  *and* its tag on GitHub (`gh release delete v0.7.0 --cleanup-tag`), then re-run the
  job or push again.
- **Two pushes close together.** Runs publish one at a time, and a run whose commit is no
  longer the tip of `main` skips publishing, so an older build never replaces a newer one.

## Things to know

- The job needs `contents: write`, granted in the workflow. The repository's default
  token permission can stay read-only.
- **Immutable releases** (repository settings) must stay off: the development build
  deletes and re-creates its release and tag on every push, which immutable releases forbid.
- Anyone who watches the repository with "Releases" turned on gets a notification for
  every development build.
- Builds are not notarized by Apple (that needs a $99/year Developer ID, see
  [platforms.md](platforms.md)), so macOS asks for permission once per new version. Every
  release page explains how.
- Pull requests build and test the app, and their builds stay in the Actions tab as
  artifacts. They are never published.
