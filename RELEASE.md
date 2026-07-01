# Release process

## Version scheme

Kaeda follows [Semantic Versioning](https://semver.org/):

- **MAJOR** — incompatible API / breaking user-facing changes.
- **MINOR** — new features in a backward-compatible manner.
- **PATCH** — backward-compatible bug fixes.

The current version is `0.2.0` (pre-1.0: minor bumps may include breaking
changes).

All version references live in `Cargo.toml` (workspace root + Tauri crate),
`package.json`, `tauri.conf.json`, and `VERSION`. Bump them all when cutting
a release.

## Creating a release

### 1. Prepare the release

```bash
# Ensure you're on main and up to date.
git checkout main && git pull

# Bump version in all manifests (see list above).
# Update the VERSION file and any changelog entries.

# Commit the version bump.
git commit -am "chore: bump version to X.Y.Z"
git push
```

### 2. Tag and push

```bash
# Tag the release.  The tag MUST start with a lowercase "v".
git tag -a "vX.Y.Z" -m "vX.Y.Z"
git push origin "vX.Y.Z"
```

### 3. CI builds and uploads

Pushing the tag triggers the **Release** workflow
(`.github/workflows/release.yml`):

| Runner      | Artifacts produced                                       |
| ----------- | -------------------------------------------------------- |
| macOS       | CLI binary (`kaeda`) + Tauri `.app` bundle + `.dmg`      |
| Linux       | CLI binary + `.deb` + `.AppImage` (all **EXPERIMENTAL**) |
| Windows     | CLI binary (`kaeda.exe`) + `.msi` + NSIS `.exe`          |

All artifacts are packed into a versioned archive
(`kaeda-vX.Y.Z-<target>.tar.gz` or `.zip`) and uploaded to the GitHub
Release draft.

Release notes are auto-generated from commits since the last tag. You can
edit them on the GitHub Releases page before publishing.

### 4. Publish the release

1. Go to **https://github.com/anomalyco/kaeda/releases**
2. Find the draft created by CI.
3. Review the auto-generated release notes — add highlights, known issues,
   important upgrade notes.
4. Publish.

## Experimental status (Linux)

Linux builds are marked **EXPERIMENTAL**. The `VERSION` metadata file and
artifact names carry this notice. Users on Linux should expect possible
issues with WebKitGTK media codec support, missing system libraries, or
window integration. Release notes should reiterate this disclaimer.

## Testing a release candidate

For pre-release testing, push a tag with a `-rc` suffix:

```bash
git tag -a "vX.Y.Z-rc1" -m "vX.Y.Z release candidate 1"
git push origin "vX.Y.Z-rc1"
```

The CI workflow will run and upload artifacts. GitHub drafts the release
(as a pre-release if you mark it during publish).
