//! Dummy target process for pt-core integration tests.
//!
//! Allocates a known pattern followed by a known `i32` value in a
//! private heap region, prints a `READY <pid> <buffer-address>` line
//! so the test can cross-check the scan result, and then sleeps
//! forever until killed. No test ever attaches to a real game; this
//! binary is the only target the test suite touches.
//!
//! Layout of the leaked buffer (little-endian on x86_64):
//!
//! ```text
//! offset 0..16:  b"PTDUMMY-PATTERN-v1"
//! offset 16..20: 1337 as i32 (the value tests read and write)
//! ```

const PATTERN: &[u8] = b"PTDUMMY-PATTERN-v1";
const VALUE: i32 = 1337;

fn main() {
    let mut buffer = Vec::with_capacity(PATTERN.len() + VALUE.to_ne_bytes().len());
    buffer.extend_from_slice(PATTERN);
    buffer.extend_from_slice(&VALUE.to_ne_bytes());
    let buffer: &'static mut [u8] = Box::leak(buffer.into_boxed_slice());

    println!(
        "READY {} {:#x}",
        std::process::id(),
        buffer.as_ptr() as usize
    );
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
