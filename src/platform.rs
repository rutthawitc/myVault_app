use std::io;
use std::path::Path;

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

