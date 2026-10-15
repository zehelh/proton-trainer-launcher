# AUR packaging

Two packages are maintained from this folder:

- `proton-trainer` — builds from the released source tarball with
  cargo.
- `proton-trainer-bin` — installs the prebuilt x86_64 binary from the
  GitHub release.

## Publishing an update

The AUR repositories (`aur.archlinux.org/packages/{proton-trainer,
proton-trainer-bin}`) are separate git repositories; these files are
the source of truth for their contents.

1. After a release is published on GitHub, update `pkgver`, `pkgrel`
   if needed, and refresh the sha256 (`SKIP` placeholders must be
   replaced with the values from the release's `SHA256SUMS`):
   `updpkgsums` or `makepkg -g`.
2. Regenerate `.SRCINFO`: `makepkg --printsrcinfo > .SRCINFO`.
3. In your clone of the AUR package repository, replace `PKGBUILD` and
   `.SRCINFO`, commit with a message like `Update to 0.1.0`, and push.

## Local verification

```bash
cd aur/proton-trainer
makepkg --printsrcinfo        # validates the PKGBUILD parses
makepkg -sf                   # full build + package test (source variant)
```
