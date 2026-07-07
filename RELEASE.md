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
(`kaeda-vX.Y.Z-<target>.tar.gz`) and uploaded to a GitHub Release that CI
**publishes directly** (no draft step). Release notes are auto-generated
from commits since the last tag.

### 4. Review the release

1. Go to **https://github.com/barroqt/kaeda/releases**
2. Open the release created by CI and confirm all four platform archives
   are attached.
3. Edit the auto-generated notes — add highlights, known issues,
   important upgrade notes.
4. Include the macOS Gatekeeper notice (see below) in the notes — macOS
   builds are ad-hoc signed, not notarized, so Gatekeeper blocks them on
   first launch.

#### macOS Gatekeeper notice (paste into release notes)

> **macOS users:** the app is not yet notarized by Apple, so macOS will
> say it "could not be verified". To open it, try launching it once, then
> go to **System Settings → Privacy & Security** and click **Open
> Anyway**. Alternatively, run
> `xattr -dr com.apple.quarantine /Applications/Kaeda.app` in Terminal.

Removing this notice (and the matching `.download-note` on the website)
requires signing with a Developer ID certificate and notarizing in CI,
which needs an Apple Developer Program membership.

### 5. Update the website download links

The download buttons on the website read their URLs from
`website/release.json` at page load. After publishing the release:

1. Edit `website/release.json` and update, for **every** entry:
   - `version` and `tag` at the top.
   - Each `assets.*.url` — replace the old tag with the new one in both
     the path segment (`download/vX.Y.Z/`) and the archive filename
     (`kaeda-vX.Y.Z-<target>.tar.gz`).
   Also update the matching static `href` on each `.btn-download` link in
   `website/index.html` (the page overrides them from `release.json` at
   load, but the static links are the no-JS/fetch-failure fallback and
   should not go stale).
2. Verify each URL downloads the right asset (open it in a browser or
   `curl -IL <url>` and check for a `200`).
3. Commit and push. Any push to `main` touching `website/**` triggers the
   **Deploy website** workflow (`.github/workflows/deploy-website.yml`),
   which republishes the site to GitHub Pages automatically.
4. Once deployed, click each download button on the live site and confirm
   it downloads the new version directly (no GitHub page in between). If
   `release.json` fails to load, the buttons fall back to the GitHub
   "latest release" page.

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

The CI workflow will run, upload artifacts, and publish the release. Mark
it as a pre-release afterwards on the GitHub Releases page.
