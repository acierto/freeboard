//! macOS-only volume details: APFS purgeable space and local snapshots,
//! reported separately because Finder's totals may include them.

use std::ffi::c_void;
use std::path::Path;

type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFURLRef = *const c_void;
type CFStringRef = *const c_void;
type CFNumberRef = *const c_void;
type Boolean = u8;
type CFIndex = isize;

const K_CF_NUMBER_SINT64_TYPE: CFIndex = 4;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFURLVolumeAvailableCapacityForImportantUsageKey: CFStringRef;
    fn CFURLCreateFromFileSystemRepresentation(
        alloc: CFAllocatorRef,
        buffer: *const u8,
        len: CFIndex,
        is_dir: Boolean,
    ) -> CFURLRef;
    fn CFURLCopyResourcePropertyForKey(url: CFURLRef, key: CFStringRef, value: *mut CFTypeRef, error: *mut CFTypeRef) -> Boolean;
    fn CFNumberGetValue(number: CFNumberRef, the_type: CFIndex, value: *mut c_void) -> Boolean;
    fn CFRelease(cf: CFTypeRef);
}

/// Space free right now, without purgeable space: `statvfs` `f_bavail`,
/// the figure `df` shows. `sysinfo` reports the important-usage capacity on
/// macOS, which already includes purgeable space (Finder's figure), so the
/// two could not be reported separately.
pub fn available_bytes(path: &Path) -> Option<u64> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: valid NUL-terminated path and a zeroed out-parameter.
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return None;
        }
        Some((st.f_bavail as u64).saturating_mul(st.f_frsize as u64))
    }
}

/// Capacity available for important usage: free space plus space macOS can
/// purge on demand (caches, local snapshots).
pub fn important_usage_capacity(path: &Path) -> Option<u64> {
    let bytes = path.as_os_str().as_encoded_bytes();
    // SAFETY: CoreFoundation calls with valid buffers; every created object is released.
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(std::ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex, 1);
        if url.is_null() {
            return None;
        }
        let mut value: CFTypeRef = std::ptr::null();
        let ok = CFURLCopyResourcePropertyForKey(
            url,
            kCFURLVolumeAvailableCapacityForImportantUsageKey,
            &mut value,
            std::ptr::null_mut(),
        );
        CFRelease(url);
        if ok == 0 || value.is_null() {
            return None;
        }
        let mut out: i64 = 0;
        let got = CFNumberGetValue(value, K_CF_NUMBER_SINT64_TYPE, &mut out as *mut i64 as *mut c_void);
        CFRelease(value);
        if got == 0 || out < 0 {
            None
        } else {
            Some(out as u64)
        }
    }
}

/// Purgeable bytes = important-usage capacity minus plain free space.
pub fn purgeable_bytes(path: &Path, free_bytes: u64) -> Option<u64> {
    important_usage_capacity(path).map(|c| c.saturating_sub(free_bytes))
}

/// Count Time Machine local snapshots from `tmutil listlocalsnapshots <mount>`.
pub fn parse_local_snapshots(out: &str) -> u32 {
    out.lines().filter(|l| l.trim_start().starts_with("com.apple.TimeMachine.")).count() as u32
}

pub fn local_snapshot_count(mount: &Path, runner: &crate::runner::Runner) -> Option<u32> {
    let out = runner
        .run_read(&crate::runner::Cmd::new("tmutil", ["listlocalsnapshots", &mount.to_string_lossy()]))
        .ok()?;
    if !out.success() {
        return None;
    }
    Some(parse_local_snapshots(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snapshot_list() {
        let out = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-11-04-101010.local\ncom.apple.TimeMachine.2026-11-04-111010.local\n";
        assert_eq!(parse_local_snapshots(out), 2);
        assert_eq!(parse_local_snapshots("Snapshots for disk /:\n"), 0);
    }

    #[test]
    fn important_usage_is_at_least_free_space() {
        let home = crate::platform::home_dir().unwrap();
        let cap = important_usage_capacity(&home).expect("CoreFoundation reports capacity");
        let free = available_bytes(&home).expect("statvfs reports free space");
        assert!(free <= cap + 1024 * 1024 * 1024, "free {free} should not exceed important-usage capacity {cap}");
        assert!(cap > 0);
    }
}
