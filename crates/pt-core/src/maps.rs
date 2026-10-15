//! Parsing and validation of `/proc/<pid>/maps`.

use std::path::Path;

use crate::error::CoreError;

/// Pathnames that must never be scanned or written even though they may
/// appear writable: they are kernel-shared mappings, not game memory.
const SKIP_PATHNAMES: &[&str] = &[
    "[vvar]",
    "[vvar_clock]",
    "[vdso]",
    "[vsyscall]",
    "[vectors]",
];

/// One mapped memory region of a process, as described by
/// `/proc/<pid>/maps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// First mapped address (inclusive).
    pub start: u64,
    /// Last mapped address (exclusive).
    pub end: u64,
    /// Whether the region is readable.
    pub readable: bool,
    /// Whether the region is writable.
    pub writable: bool,
    /// Whether the region is executable.
    pub executable: bool,
    /// Whether the mapping is private (`p`) rather than shared (`s`).
    pub private: bool,
    /// The mapped file, if any (e.g. `/usr/lib/...` or `[heap]`).
    pub pathname: Option<String>,
}

impl Region {
    /// Region size in bytes.
    pub fn size(&self) -> u64 {
        self.end - self.start
    }

    /// Whether this region should be scanned for byte patterns:
    /// private writable mappings that are not kernel-shared.
    ///
    /// This is deliberately conservative: anonymous memory, `[heap]`,
    /// and module-backed private data are game memory; shared and
    /// kernel mappings are not.
    pub fn is_scan_candidate(&self) -> bool {
        if !(self.readable && self.writable && self.private) {
            return false;
        }
        match &self.pathname {
            None => true,
            Some(name) => !SKIP_PATHNAMES.iter().any(|skip| name.contains(skip)),
        }
    }
}

/// Reads and parses `/proc/<pid>/maps`.
///
/// # Errors
///
/// Returns [`CoreError::ProcessGone`] when the process no longer exists
/// (its `/proc` entry is gone), [`CoreError::Io`] for other read
/// failures, and [`CoreError::MapsParse`] for malformed lines.
pub fn read_maps(pid: i32) -> Result<Vec<Region>, CoreError> {
    let path = Path::new("/proc").join(pid.to_string()).join("maps");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(source)
            if matches!(source.kind(), std::io::ErrorKind::NotFound)
                || source.raw_os_error() == Some(libc::ESRCH) =>
        {
            return Err(CoreError::ProcessGone { pid });
        }
        Err(source) => return Err(CoreError::Io { path, source }),
    };
    parse_maps(pid, &text)
}

/// Parses the content of a `/proc/<pid>/maps` file.
///
/// # Errors
///
/// Returns [`CoreError::MapsParse`] when a line is malformed.
pub fn parse_maps(pid: i32, text: &str) -> Result<Vec<Region>, CoreError> {
    let mut regions = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        regions.push(parse_line(pid, index + 1, line)?);
    }
    Ok(regions)
}

/// Parses one maps line:
/// `start-end perms offset dev inode [pathname]`.
fn parse_line(pid: i32, line: usize, text: &str) -> Result<Region, CoreError> {
    let parse_error = |message: String| CoreError::MapsParse { pid, line, message };

    let mut cursor = Cursor::new(text);
    let range = cursor.next_token();
    let Some((start, end)) = range.split_once('-') else {
        return Err(parse_error(format!(
            "expected 'start-end', found {range:?}"
        )));
    };
    let start = u64::from_str_radix(start, 16)
        .map_err(|_| parse_error(format!("invalid start address {start:?}")))?;
    let end = u64::from_str_radix(end, 16)
        .map_err(|_| parse_error(format!("invalid end address {end:?}")))?;
    if end <= start {
        return Err(parse_error(format!("end {end:#x} <= start {start:#x}")));
    }

    let perms = cursor.next_token();
    if perms.len() != 4
        || !perms
            .chars()
            .take(3)
            .all(|c| matches!(c, 'r' | 'w' | 'x' | '-'))
    {
        return Err(parse_error(format!("invalid permissions {perms:?}")));
    }

    // offset, device, inode: parsed for validation only.
    for field in ["offset", "device", "inode"] {
        let token = cursor.next_token();
        if token.is_empty() {
            return Err(parse_error(format!("missing {field} field")));
        }
    }

    let pathname = cursor.remainder();
    let region = Region {
        start,
        end,
        readable: perms.starts_with('r'),
        writable: perms.get(1..2) == Some("w"),
        executable: perms.get(2..3) == Some("x"),
        private: perms.ends_with('p'),
        pathname: if pathname.is_empty() {
            None
        } else {
            Some(pathname.to_string())
        },
    };
    Ok(region)
}

/// Whitespace-aware field cursor over one maps line. Pathnames may
/// contain spaces, so only the first five fields are tokenized; the
/// remainder is kept verbatim.
struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self { rest: text }
    }

    /// Consumes and returns the next whitespace-delimited token.
    fn next_token(&mut self) -> &'a str {
        self.rest = self.rest.trim_start();
        let end = self
            .rest
            .find(char::is_whitespace)
            .unwrap_or(self.rest.len());
        let token = &self.rest[..end];
        self.rest = &self.rest[end..];
        token
    }

    /// Returns everything after the last consumed token, trimmed.
    fn remainder(&self) -> &'a str {
        self.rest.trim_start()
    }
}

/// Validates that `[start, end)` lies fully inside one mapped writable
/// region of the process, and returns that region.
///
/// Region lists are re-parsed at the moment of use (plan and apply),
/// never cached across stops, so a target that re-maps memory between
/// dry-run and apply is caught here.
///
/// # Errors
///
/// Returns [`CoreError::RegionNotWritable`] when `start` is not inside
/// a mapped writable region, and [`CoreError::RegionSpans`] when the
/// range would cross the end of that region.
pub fn validate_writable_range(
    regions: &[Region],
    pid: i32,
    start: u64,
    end: u64,
) -> Result<&Region, CoreError> {
    if end <= start {
        return Err(CoreError::RegionSpans { pid, start, end });
    }
    let Some(region) = regions
        .iter()
        .find(|region| region.start <= start && start < region.end)
    else {
        return Err(CoreError::RegionNotWritable { pid, addr: start });
    };
    if !region.writable {
        return Err(CoreError::RegionNotWritable { pid, addr: start });
    }
    if end > region.end {
        return Err(CoreError::RegionSpans { pid, start, end });
    }
    Ok(region)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPS: &str = "\
00400000-00410000 r--p 00000000 00:05 1  /usr/game/game.exe
01230000-01234000 rw-p 00000000 00:05 2  [heap]
7f0000000000-7f0000010000 rw-p 00000000 00:00 3  
7ffff0000000-7ffff0010000 r-xs 00000000 00:05 4  /dev/shm/shared
7ffff7fcf000-7ffff7fd0000 r--p 00000000 00:05 5  [vvar]
";

    #[test]
    fn parses_all_lines() {
        let regions = parse_maps(42, MAPS).unwrap();
        assert_eq!(regions.len(), 5);

        assert_eq!(regions[1].pathname.as_deref(), Some("[heap]"));
        assert!(regions[1].writable && regions[1].private && regions[1].is_scan_candidate());

        // Anonymous writable region (no pathname field).
        assert_eq!(regions[2].pathname, None);
        assert!(regions[2].is_scan_candidate());

        // Shared mapping: never a scan candidate.
        assert!(!regions[3].private);

        // Kernel mapping: excluded from scans despite being readable.
        assert_eq!(regions[4].pathname.as_deref(), Some("[vvar]"));
        assert!(!regions[4].is_scan_candidate());
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_maps(42, "garbage").is_err());
        assert!(parse_maps(42, "0000-0001 zz-p 0 00:05 1").is_err());
        assert!(parse_maps(42, "0010-0001 rw-p 0 00:05 1").is_err());
    }

    #[test]
    fn validates_writable_ranges() {
        let regions = parse_maps(42, MAPS).unwrap();
        let heap = &regions[1];

        // Fully inside the heap region.
        assert_eq!(
            validate_writable_range(&regions, 42, heap.start, heap.start + 8).unwrap(),
            heap
        );

        // Read-only region: refused.
        assert!(matches!(
            validate_writable_range(&regions, 42, regions[0].start, regions[0].start + 4),
            Err(CoreError::RegionNotWritable { .. })
        ));

        // Unmapped address: refused.
        assert!(matches!(
            validate_writable_range(&regions, 42, 0x1000, 0x1004),
            Err(CoreError::RegionNotWritable { .. })
        ));

        // Range crossing the end of the heap region: refused.
        assert!(matches!(
            validate_writable_range(&regions, 42, heap.end - 2, heap.end + 2),
            Err(CoreError::RegionSpans { .. })
        ));

        // Empty range: refused.
        assert!(matches!(
            validate_writable_range(&regions, 42, heap.start, heap.start),
            Err(CoreError::RegionSpans { .. })
        ));
    }
}
