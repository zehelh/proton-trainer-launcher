//! Integration tests exercising the full engine against the dummy
//! target process: discovery, scan, read, plan (dry-run), apply
//! (verified write), refusals, and stop/restore guarantees.
//!
//! No test here ever attaches to a real game or to Steam.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use pt_core::engine::{apply, plan_write};
use pt_core::error::CoreError;
use pt_core::maps::read_maps;
use pt_core::mem::read_bytes;
use pt_core::pattern::{scan_process, Pattern};
use pt_core::process::find_processes;
use pt_core::stop::{process_state, StopGuard};

const PATTERN: &[u8] = b"PTDUMMY-PATTERN-v1";
const INITIAL_VALUE: i32 = 1337;
const NEW_VALUE: i32 = 4242;

/// Kills the dummy on drop so a failing or panicking test never leaks
/// the child process.
struct Dummy {
    child: Child,
    /// Address of the leaked pattern buffer in the dummy.
    buffer_addr: u64,
}

impl Dummy {
    fn pid(&self) -> i32 {
        self.child.id() as i32
    }
}

impl Drop for Dummy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawns the dummy target and waits for its READY line.
fn spawn_dummy() -> Dummy {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pt-dummy-target"))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn the dummy target");

    let stdout = child.stdout.take().expect("dummy stdout was not piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("dummy did not print a READY line");

    let mut parts = line.split_whitespace();
    let ready = parts.next().expect("empty READY line");
    assert_eq!(ready, "READY", "unexpected READY line: {line:?}");
    let pid: u32 = parts
        .next()
        .expect("missing pid in READY line")
        .parse()
        .expect("invalid pid in READY line");
    assert_eq!(pid, child.id(), "dummy pid mismatch");
    let buffer_addr = u64::from_str_radix(
        parts
            .next()
            .expect("missing buffer address in READY line")
            .trim_start_matches("0x"),
        16,
    )
    .expect("invalid buffer address in READY line");

    Dummy { child, buffer_addr }
}

#[test]
fn discovers_and_scans_the_dummy() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    // Discovery by exe basename.
    let found = find_processes("pt-dummy-target").unwrap();
    assert!(
        found.iter().any(|p| p.pid == pid),
        "dummy {pid} not found by exe name"
    );

    // Scan finds exactly one match, at the address the dummy reported.
    let pattern = Pattern::literal(PATTERN.to_vec()).unwrap();
    let matches = scan_process(pid, &pattern).unwrap();
    assert_eq!(matches, vec![dummy.buffer_addr]);
}

#[test]
fn reads_the_known_value() {
    let dummy = spawn_dummy();
    let value_addr = dummy.buffer_addr + PATTERN.len() as u64;

    let mut buffer = [0u8; 4];
    read_bytes(dummy.pid(), value_addr, &mut buffer).unwrap();
    let value = i32::from_ne_bytes(buffer);
    assert_eq!(value, INITIAL_VALUE);
}

#[test]
fn dry_run_plan_performs_no_write() {
    let dummy = spawn_dummy();
    let value_addr = dummy.buffer_addr + PATTERN.len() as u64;

    let plan = plan_write(dummy.pid(), value_addr, &NEW_VALUE.to_ne_bytes()).unwrap();
    assert_eq!(plan.old_bytes, INITIAL_VALUE.to_ne_bytes());
    assert_eq!(plan.new_bytes, NEW_VALUE.to_ne_bytes());
    assert!(plan.region.writable);
    assert!(plan.region.start <= value_addr && value_addr + 4 <= plan.region.end);

    // The plan must not have changed the target's memory.
    let mut buffer = [0u8; 4];
    read_bytes(dummy.pid(), value_addr, &mut buffer).unwrap();
    assert_eq!(i32::from_ne_bytes(buffer), INITIAL_VALUE);
}

#[test]
fn apply_writes_and_verifies_without_leaking_a_stop() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();
    let value_addr = dummy.buffer_addr + PATTERN.len() as u64;

    let plan = plan_write(pid, value_addr, &NEW_VALUE.to_ne_bytes()).unwrap();
    apply(&plan).unwrap();

    // Fresh read through the engine's public API.
    let mut buffer = [0u8; 4];
    read_bytes(pid, value_addr, &mut buffer).unwrap();
    assert_eq!(i32::from_ne_bytes(buffer), NEW_VALUE);

    // The target must be running again, not stopped.
    let state = process_state(pid).unwrap();
    assert_ne!(state, 'T', "target left stopped after apply");
    assert_ne!(state, 'Z', "target died during apply");
}

#[test]
fn plan_refuses_unmapped_and_readonly_addresses() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    // A low, surely-unmapped address.
    let error = plan_write(pid, 0x1000, &[1, 2, 3, 4]).unwrap_err();
    assert!(matches!(error, CoreError::RegionNotWritable { .. }));

    // A read-only region: find one from the dummy's maps.
    let regions = read_maps(pid).unwrap();
    let readonly = regions
        .iter()
        .find(|r| r.readable && !r.writable)
        .expect("dummy has no read-only region");
    let error = plan_write(pid, readonly.start, &[1, 2, 3, 4]).unwrap_err();
    assert!(matches!(error, CoreError::RegionNotWritable { .. }));
}

#[test]
fn plan_refuses_ranges_crossing_region_boundaries() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    let regions = read_maps(pid).unwrap();
    let writable = regions
        .iter()
        .find(|r| r.writable && r.private)
        .expect("dummy has no writable region");
    // Four bytes starting two bytes before the region's end cross it.
    let error = plan_write(pid, writable.end - 2, &[1, 2, 3, 4]).unwrap_err();
    assert!(matches!(error, CoreError::RegionSpans { .. }));
}

#[test]
fn plan_refuses_empty_writes() {
    let dummy = spawn_dummy();
    let error = plan_write(dummy.pid(), dummy.buffer_addr, &[]).unwrap_err();
    assert!(matches!(error, CoreError::EmptyWrite));
}

#[test]
fn stop_guard_stops_and_resumes() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    let mut guard = StopGuard::stop(pid).unwrap();
    assert!(guard.is_stopped());
    assert_eq!(process_state(pid).unwrap(), 'T');

    guard.resume().unwrap();
    assert!(!guard.is_stopped());
    // Wait for the running state to come back.
    wait_for_state(pid, |state| state != 'T');
}

#[test]
fn stop_guard_resumes_on_drop_without_explicit_resume() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    {
        let _guard = StopGuard::stop(pid).unwrap();
        assert_eq!(process_state(pid).unwrap(), 'T');
    }
    wait_for_state(pid, |state| state != 'T');
}

#[test]
fn stop_guard_refuses_an_already_stopped_process() {
    let dummy = spawn_dummy();
    let pid = dummy.pid();

    let mut outer = StopGuard::stop(pid).unwrap();
    let error = StopGuard::stop(pid).unwrap_err();
    assert!(matches!(error, CoreError::AlreadyStopped { .. }));
    outer.resume().unwrap();
    wait_for_state(pid, |state| state != 'T');
}

#[test]
fn operations_on_a_dead_process_report_process_gone() {
    let mut dummy = spawn_dummy();
    let pid = dummy.pid();
    let value_addr = dummy.buffer_addr + PATTERN.len() as u64;

    dummy.child.kill().unwrap();
    dummy.child.wait().unwrap();

    let error = plan_write(pid, value_addr, &NEW_VALUE.to_ne_bytes()).unwrap_err();
    match error {
        CoreError::ProcessGone { pid: gone } => assert_eq!(gone, pid),
        other => panic!("expected ProcessGone, got: {other}"),
    }
}

/// Polls the target state until `predicate` holds (2 s budget).
fn wait_for_state(pid: i32, predicate: impl Fn(char) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if let Ok(state) = process_state(pid) {
            if predicate(state) {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("target {pid} did not reach the expected state in time");
}
