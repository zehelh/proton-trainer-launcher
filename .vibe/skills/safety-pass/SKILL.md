---
name: safety-pass
description: Review all memory-write code paths for safety, signal handling, and restore guarantees before a release.
user-invocable: true
allowed-tools:
  - read_file
  - grep
  - bash
---

# Safety pass

1. List every code path that reads or writes another process's memory.
2. For each, verify: mapped-region validation BEFORE write, read-back
   verification AFTER write, SIGSTOP/SIGCONT restore on ALL error paths
   (including panics — check the unwind path), no hardcoded pid assumptions.
3. grep the workspace for `unwrap|expect|unsafe` outside tests: justify or
   remove every hit.
4. Run `cargo test --all-features` and the proptest suite.
5. Output a findings table (severity, file, finding, fix), then implement
   fixes with regression tests.