use std::fs::Metadata;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetCompressedFileSizeW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, INVALID_FILE_SIZE, OPEN_EXISTING,
};

use super::FileId;

fn wide(path: &Path) -> Vec<u16> {
    // Long-path support: prefix absolute paths with \\?\. Windows does not
    // normalize verbatim paths, so `/` separators become `\\` first (paths
    // joined from settings or tests may contain them). `..` never gets here:
    // the guard rejects it.
    let s = path.as_os_str();
    let mut v: Vec<u16> = if path.is_absolute() && !s.to_string_lossy().starts_with(r"\\") {
        std::ffi::OsStr::new(r"\\?\")
            .encode_wide()
            .chain(s.encode_wide().map(|c| if c == b'/' as u16 { b'\\' as u16 } else { c }))
            .collect()
    } else {
        s.encode_wide().collect()
    };
    v.push(0);
    v
}

/// Actual on-disk size via GetCompressedFileSizeW (handles compressed and sparse files).
pub fn allocated_size(path: &Path, meta: &Metadata) -> u64 {
    if meta.is_dir() {
        return 0;
    }
    let w = wide(path);
    let mut high: u32 = 0;
    // SAFETY: `w` is a valid NUL-terminated wide string; `high` is a valid out pointer.
    let low = unsafe { GetCompressedFileSizeW(w.as_ptr(), &mut high) };
    if low == INVALID_FILE_SIZE && std::io::Error::last_os_error().raw_os_error().unwrap_or(0) != 0 {
        return meta.len();
    }
    ((high as u64) << 32) | low as u64
}

fn by_handle_info(path: &Path) -> Option<BY_HANDLE_FILE_INFORMATION> {
    let w = wide(path);
    // SAFETY: valid wide path; flags open the link itself, not its target.
    unsafe {
        let h = CreateFileW(
            w.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        let ok = GetFileInformationByHandle(h, &mut info);
        CloseHandle(h);
        if ok == 0 {
            None
        } else {
            Some(info)
        }
    }
}

pub fn file_id(path: &Path, _meta: &Metadata) -> Option<FileId> {
    let info = by_handle_info(path)?;
    Some(FileId {
        dev: info.dwVolumeSerialNumber as u64,
        ino: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
    })
}

pub fn link_count(path: &Path, _meta: &Metadata) -> u64 {
    by_handle_info(path).map(|i| i.nNumberOfLinks as u64).unwrap_or(1)
}

/// Allocated size, link count and identity of a file. Each Windows call opens
/// the file, so link count and identity come from one handle: walking a
/// million-file tree with separate calls took 69 s on a Windows Arm64 runner
/// (2026-11-08). The identity is only returned for hard-linked files.
pub fn file_facts(path: &Path, meta: &Metadata) -> (u64, u64, Option<FileId>) {
    let size = allocated_size(path, meta);
    match by_handle_info(path) {
        Some(i) if i.nNumberOfLinks > 1 => (
            size,
            i.nNumberOfLinks as u64,
            Some(FileId { dev: i.dwVolumeSerialNumber as u64, ino: ((i.nFileIndexHigh as u64) << 32) | i.nFileIndexLow as u64 }),
        ),
        Some(_) => (size, 1, None),
        None => (size, 1, None),
    }
}

/// Junctions and all other reparse points count as links.
pub fn is_link_meta(_path: &Path, meta: &Metadata) -> bool {
    meta.file_type().is_symlink() || (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

fn filetime_to_unix(ft: u64) -> i64 {
    (ft / 10_000_000) as i64 - 11_644_473_600
}

pub fn modified_secs(meta: &Metadata) -> i64 {
    filetime_to_unix(meta.last_write_time())
}

pub fn accessed_secs(meta: &Metadata) -> i64 {
    filetime_to_unix(meta.last_access_time())
}

pub fn system_protected() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let root = PathBuf::from(format!("{drive}\\"));
    let mut exact = vec![root.clone(), root.join("Users")];
    if let Ok(t) = std::env::var("TEMP") {
        exact.push(PathBuf::from(t));
    }
    let mut subtree: Vec<PathBuf> = ["Windows", "Program Files", "Program Files (x86)", "ProgramData", "$Recycle.Bin"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    if let Ok(w) = std::env::var("SystemRoot") {
        subtree.push(PathBuf::from(w));
    }
    (exact, subtree)
}

/// Process working directories are not readable without elevated APIs on
/// Windows; report unknown so callers raise risk.
pub fn processes_in_dir(_dir: &Path, _runner: &crate::runner::Runner) -> Option<Vec<u32>> {
    None
}

pub fn process_cwds(_runner: &crate::runner::Runner) -> Option<Vec<(u32, PathBuf)>> {
    None
}

pub(super) fn process_alive_impl(pid: u32) -> Option<bool> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: plain Win32 calls; the handle is closed.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return match GetLastError() {
                ERROR_INVALID_PARAMETER => Some(false),
                ERROR_ACCESS_DENIED => Some(true),
                _ => None,
            };
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        if ok == 0 {
            None
        } else {
            Some(code == STILL_ACTIVE as u32)
        }
    }
}

pub(super) fn on_battery_impl() -> Option<bool> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // SAFETY: valid out pointer.
    unsafe {
        let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut s) == 0 {
            return None;
        }
        match s.ACLineStatus {
            0 => Some(true),
            1 => Some(false),
            _ => None,
        }
    }
}

pub fn lower_thread_priority() {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
    // SAFETY: pseudo-handle of the current thread.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}
