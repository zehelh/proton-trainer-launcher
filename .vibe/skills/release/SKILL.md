---
name: release
description: Cut a release: checks, changelog, tag, CI builds (binary, AppImage, AUR bump), SHA256SUMS.
user-invocable: true
---

# Release

1. Full gate: fmt --check, clippy -D warnings, tests --all-features, doc build,
   cargo deny check.
2. Run the /safety-pass and /compat checklists. Fix or explicitly defer
   findings.
3. Update CHANGELOG.md (Keep a Changelog, English), bump version in
   Cargo.toml (workspace), update the AUR PKGBUILD (pkgver + sha256 for -bin).
4. Commit `chore(release): vX.Y.Z`, tag, push.
5. Verify CI produced: x86_64 binary, AppImage, SHA256SUMS. Draft the GitHub
   release notes from the changelog, including install instructions for
   CachyOS (AUR) and Bazzite (AppImage).