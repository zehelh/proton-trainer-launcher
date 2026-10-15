//! Byte-pattern scanning of the target's memory.
//!
//! Patterns are matched against the target's private writable regions
//! (see [`crate::maps::Region::is_scan_candidate`]) in fixed-size
//! chunks with an overlap so matches spanning chunk boundaries are
//! found. Reads that fail mid-scan (the target unmapped a region
//! while running) skip the affected chunk instead of failing the scan.

use crate::error::CoreError;
use crate::maps::{read_maps, Region};
use crate::mem::read_bytes;

/// Read chunk size for scanning: 256 KiB per `process_vm_readv` call.
const CHUNK_SIZE: u64 = 256 * 1024;

/// A byte pattern with a per-byte match mask.
///
/// `mask[i] == true` means `bytes[i]` must match; `false` is a
/// wildcard. Used by trainer definitions (milestone P3) and by the
/// scan engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The bytes to match.
    pub bytes: Vec<u8>,
    /// Per-byte mask: `true` = must match, `false` = wildcard.
    pub mask: Vec<bool>,
}

impl Pattern {
    /// Builds a pattern from bytes and a mask of the same length.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPattern`] when the pattern is empty
    /// or the mask length differs from the byte count.
    pub fn new(bytes: Vec<u8>, mask: Vec<bool>) -> Result<Self, CoreError> {
        if bytes.is_empty() {
            return Err(CoreError::InvalidPattern(
                "the pattern is empty".to_string(),
            ));
        }
        if bytes.len() != mask.len() {
            return Err(CoreError::InvalidPattern(format!(
                "mask length {} differs from bytes length {}",
                mask.len(),
                bytes.len()
            )));
        }
        Ok(Self { bytes, mask })
    }

    /// Builds a pattern where every byte must match (no wildcards).
    ///
    /// # Errors
    ///
    /// Returns an error for empty input.
    pub fn literal(bytes: Vec<u8>) -> Result<Self, CoreError> {
        let len = bytes.len();
        Self::new(bytes, vec![true; len])
    }

    /// Pattern length in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the pattern is empty (it never is; `new` rejects empty).
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Returns whether `pattern` matches `data` starting at `data[offset]`.
fn matches_at(pattern: &Pattern, data: &[u8], offset: usize) -> bool {
    pattern
        .bytes
        .iter()
        .zip(pattern.mask.iter())
        .enumerate()
        .all(|(i, (byte, must_match))| !must_match || data.get(offset + i) == Some(byte))
}

/// Scans one in-memory slice for `pattern`, returning the addresses of
/// all matches relative to `base_addr`.
fn scan_slice(pattern: &Pattern, data: &[u8], base_addr: u64) -> Vec<u64> {
    let mut matches = Vec::new();
    if data.len() < pattern.len() {
        return matches;
    }
    for offset in 0..=(data.len() - pattern.len()) {
        if matches_at(pattern, data, offset) {
            matches.push(base_addr + offset as u64);
        }
    }
    matches
}

/// Scans all candidate regions of `pid` for `pattern`, returning every
/// matching address.
///
/// The scan reads while the target is running (SIGSTOP is only used
/// around writes); chunks that fail to read are skipped, which keeps
/// the scan resilient to a live target re-arranging its memory.
///
/// # Errors
///
/// Returns [`CoreError::Io`] when `/proc/<pid>/maps` cannot be read.
pub fn scan_process(pid: i32, pattern: &Pattern) -> Result<Vec<u64>, CoreError> {
    let regions = read_maps(pid)?;
    let mut matches = Vec::new();
    for region in regions.iter().filter(|region| region.is_scan_candidate()) {
        matches.extend(scan_region(pid, region, pattern));
    }
    matches.sort_unstable();
    Ok(matches)
}

/// Scans one region in chunks with overlap.
fn scan_region(pid: i32, region: &Region, pattern: &Pattern) -> Vec<u64> {
    let mut matches = Vec::new();
    let pattern_len = pattern.len() as u64;
    let mut chunk_start = region.start;
    while chunk_start < region.end {
        let chunk_end = (chunk_start + CHUNK_SIZE).min(region.end);
        let size = (chunk_end - chunk_start) as usize;
        let mut chunk = vec![0u8; size];
        match read_bytes(pid, chunk_start, &mut chunk) {
            Ok(()) => {
                matches.extend(scan_slice(pattern, &chunk, chunk_start));
                // Overlap so a pattern straddling the boundary is found.
                chunk_start = chunk_end
                    .saturating_sub(pattern_len - 1)
                    .max(chunk_start + 1);
            }
            Err(error) => {
                tracing::debug!(
                    region_start = chunk_start,
                    %error,
                    "skipping unreadable chunk during scan"
                );
                chunk_start = chunk_end;
            }
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_pattern_matches_only_exact_bytes() {
        let pattern = Pattern::literal(b"ABCD".to_vec()).unwrap();
        assert!(matches_at(&pattern, b"xxABCD", 2));
        assert!(!matches_at(&pattern, b"ABXD", 0));
    }

    #[test]
    fn wildcard_pattern_matches_any_byte_at_masked_off_positions() {
        let pattern = Pattern::new(b"ABC".to_vec(), vec![true, true, false]).unwrap();
        assert!(matches_at(&pattern, b"AB1", 0));
        assert!(matches_at(&pattern, b"ABZ", 0));
        assert!(!matches_at(&pattern, b"AX1", 0));
    }

    #[test]
    fn patterns_must_be_non_empty_and_aligned() {
        assert!(Pattern::new(vec![], vec![]).is_err());
        assert!(Pattern::new(b"A".to_vec(), vec![]).is_err());
    }

    #[test]
    fn scan_slice_finds_all_occurrences_with_addresses() {
        let pattern = Pattern::literal(b"ab".to_vec()).unwrap();
        assert_eq!(scan_slice(&pattern, b"abab", 0x1000), vec![0x1000, 0x1002]);
        assert!(scan_slice(&pattern, b"a", 0).is_empty());
    }

    #[test]
    fn scan_slice_finds_matches_across_chunk_boundaries() {
        // Pattern placed at the very end of one chunk.
        let pattern = Pattern::literal(b"XYZW".to_vec()).unwrap();
        let data = vec![0u8; 8];
        let mut data = data;
        data[4..8].copy_from_slice(b"XYZW");
        assert_eq!(scan_slice(&pattern, &data, 0x2000), vec![0x2004]);
    }
}
