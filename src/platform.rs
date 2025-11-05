use std::io;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
pub fn hide(path: &Path) -> io::Result<()> {
    use winapi::um::fileapi::GetFileAttributesW;
    use winapi::um::fileapi::SetFileAttributesW;
    use winapi::um::winnt::FILE_ATTRIBUTE_HIDDEN;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFFFFFF;

    // Convert path to wide string for Windows API
    let wide: Vec<u16> = OsStr::new(path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr() as *const u16);
        if attrs == INVALID_FILE_ATTRIBUTES {
            return Err(io::Error::last_os_error());
        }

        let new_attrs = attrs | FILE_ATTRIBUTE_HIDDEN;
        if SetFileAttributesW(wide.as_ptr() as *const u16, new_attrs) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn unhide(path: &Path) -> io::Result<()> {
    use winapi::um::fileapi::GetFileAttributesW;
    use winapi::um::fileapi::SetFileAttributesW;
    use winapi::um::winnt::FILE_ATTRIBUTE_HIDDEN;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFFFFFF;

    // Convert path to wide string for Windows API
    let wide: Vec<u16> = OsStr::new(path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr() as *const u16);
        if attrs == INVALID_FILE_ATTRIBUTES {
            return Err(io::Error::last_os_error());
        }

        let new_attrs = attrs & !FILE_ATTRIBUTE_HIDDEN;
        if SetFileAttributesW(wide.as_ptr() as *const u16, new_attrs) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn hide(_path: &Path) -> io::Result<()> { Ok(()) }

#[cfg(not(target_os = "windows"))]
pub fn unhide(_path: &Path) -> io::Result<()> { Ok(()) }

/// Get the platform identifier
#[cfg(target_os = "windows")]
pub fn platform() -> &'static str {
    "windows"
}

#[cfg(target_os = "macos")]
pub fn platform() -> &'static str {
    "macos"
}

#[cfg(target_os = "linux")]
pub fn platform() -> &'static str {
    "linux"
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn platform() -> &'static str {
    "unknown"
}

/// Get the configuration directory path
/// Windows: C:\Users\<User>\AppData\Local\MyVault\
/// macOS: ~/Library/Application Support/MyVault/
/// Linux: ~/.local/share/myvault/
pub fn config_dir() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine config directory"))
            .map(|p| p.join("MyVault"))
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine config directory"))
            .map(|p| p.join("MyVault"))
    }

    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine config directory"))
            .map(|p| p.join("myvault"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // Fallback: use current directory
        std::env::current_dir()
    }
}

/// Get the cache directory path
pub fn cache_dir() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::cache_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine cache directory"))
            .map(|p| p.join("MyVault"))
    }

    #[cfg(target_os = "macos")]
    {
        dirs::cache_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine cache directory"))
            .map(|p| p.join("MyVault"))
    }

    #[cfg(target_os = "linux")]
    {
        dirs::cache_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not determine cache directory"))
            .map(|p| p.join("myvault"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // Fallback: use temp directory
        Ok(std::env::temp_dir()
            .join("myvault"))
    }
}

/// Check if running on Windows
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Check if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Check if running on Linux
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Phase 1: Copy text to clipboard
#[cfg(target_os = "windows")]
pub fn set_clipboard(text: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::um::winuser::{OpenClipboard, EmptyClipboard, SetClipboardData, CloseClipboard};
    use winapi::um::winbase::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    unsafe {
        // Open clipboard
        if OpenClipboard(ptr::null_mut()) == 0 {
            return Err("Failed to open clipboard".to_string());
        }

        // Empty clipboard
        if EmptyClipboard() == 0 {
            CloseClipboard();
            return Err("Failed to empty clipboard".to_string());
        }

        // Convert text to wide string (UTF-16)
        let wide: Vec<u16> = OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let len = wide.len() * std::mem::size_of::<u16>();

        // Allocate global memory
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, len);
        if h_mem.is_null() {
            CloseClipboard();
            return Err("Failed to allocate memory".to_string());
        }

        // Lock memory and copy data
        let p_mem = GlobalLock(h_mem);
        if p_mem.is_null() {
            CloseClipboard();
            return Err("Failed to lock memory".to_string());
        }

        ptr::copy_nonoverlapping(wide.as_ptr(), p_mem as *mut u16, wide.len());
        GlobalUnlock(h_mem);

        // Set clipboard data (CF_UNICODETEXT = 13)
        const CF_UNICODETEXT: u32 = 13;
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            CloseClipboard();
            return Err("Failed to set clipboard data".to_string());
        }

        CloseClipboard();
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_clipboard(_text: &str) -> Result<(), String> {
    Err("Clipboard not supported on this platform yet".to_string())
}

