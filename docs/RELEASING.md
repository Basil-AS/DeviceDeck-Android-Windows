# DeviceDeck releases (Android, Windows and Linux)

The Android/desktop integration branch is `develop`; production release tags are created only from commits merged to `main`. Keep the release version aligned in `app/build.gradle.kts`, `desktop-tauri/Cargo.toml` and `desktop-tauri/tauri.conf.json`.

## Build and publish

1. Prepare a release branch from `develop`, update version and changelog, run the complete Build workflow, and merge the reviewed PR to `develop`.
2. Merge the release commit to `main` through a GitFlow PR.
3. Push one platform tag matching the declared version:
   - `android-v<version>` builds APK/AAB, requires the persistent signer, verifies the APK, and publishes Android assets plus SHA256SUMS.
   - `desktop-v<version>` builds Windows x64 ZIP and Linux x64 AppImage, then publishes those assets plus SHA256SUMS.
4. GitHub Actions rejects tags that do not point to a commit reachable from `main` or whose tag/version does not match the application metadata.
5. Alpha/beta/rc versions are GitHub prereleases. Stable versions become the latest release.

The tag workflow runs checks and builds before publication. Artifact checksums are generated from the exact release build. Android signing secrets are exposed only to `android-v*` tags, not pull requests or ordinary branch builds.

## Android signing key custody

The long-lived Android signing key is kept out of Git. The local working copy and Gradle properties are in ignored `local/keys/`; the same key is stored in repository Actions secrets `RELEASE_KEYSTORE_B64` and `RELEASE_KEY_PROPERTIES`. Back up `local/keys/quickbuds-release.jks` and `local/keys/keystore.properties` to an owner-controlled encrypted backup. If the private key is lost, users cannot install later updates over APKs signed by it.

## macOS

See [macOS release instructions](https://github.com/Basil-AS/DeviceDeck-macOS/blob/develop/docs/RELEASING.md). Current macOS artifacts are ad-hoc signed and not notarized. A Developer ID certificate and Apple notarization credentials are required for a Gatekeeper-clean release.
