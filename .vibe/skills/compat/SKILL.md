---
name: compat
description: Run the CachyOS/Bazzite compatibility checklist against the current tree.
user-invocable: true
---

# Compatibility checklist

1. Steam discovery: native path (~/.steam/steam, ~/.local/share/Steam) AND
   Flatpak path (~/.var/app/com.valvesoftware.Steam). Verify both are parsed
   from real fixture vdf/appmanifest files in tests.
2. ptrace_scope: simulate scope 0/1/2 and verify the error messages are
   actionable (exact sysctl/setcap commands printed).
3. No host mutations: the app must run read-only against Steam data; confirm
   no code writes inside Steam directories.
4. AppImage checklist: no hard dependency on distro packages outside glibc
   baseline; verify the AppImage builds in CI and runs in the Arch container
   with a $HOME sandbox.
5. Report every gap as a checklist table with status and file references.