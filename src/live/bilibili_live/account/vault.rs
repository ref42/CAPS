//! Per-user Windows DPAPI storage; never fall back to plaintext.
use super::Credentials;
use std::{ffi::c_void, path::PathBuf};
#[repr(C)]
struct Blob {
    size: u32,
    data: *mut u8,
}
#[link(name = "crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        input: *mut Blob,
        description: *const u16,
        entropy: *mut Blob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        output: *mut Blob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *mut Blob,
        description: *mut *mut u16,
        entropy: *mut Blob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        output: *mut Blob,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}
fn path() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA").or_else(|| std::env::var_os("LOCALAPPDATA"))?)
            .join("CAPS")
            .join("bilibili-session.bin"),
    )
}
fn protect(bytes: &[u8], decrypt: bool) -> Option<Vec<u8>> {
    let mut input = Blob {
        size: bytes.len().try_into().ok()?,
        data: bytes.as_ptr() as *mut u8,
    };
    let mut output = Blob {
        size: 0,
        data: std::ptr::null_mut(),
    };
    unsafe {
        let ok = if decrypt {
            CryptUnprotectData(
                &mut input,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        } else {
            CryptProtectData(
                &mut input,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        };
        if ok == 0 {
            return None;
        }
        let result = std::slice::from_raw_parts(output.data, output.size as usize).to_vec();
        LocalFree(output.data.cast());
        Some(result)
    }
}
pub(super) fn load() -> Option<Credentials> {
    let path = path()?;
    if std::fs::metadata(&path).ok()?.len() > 32 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 32 * 1024 {
        return None;
    }
    serde_json::from_slice(&protect(&bytes, true)?).ok()
}
pub(super) fn save(credentials: &Credentials) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(credentials).map_err(|_| "Could not protect the sign-in session")?;
    let encrypted = protect(&bytes, false)
        .ok_or("Windows could not protect this session. Please try again.")?;
    let path = path().ok_or("Session storage is unavailable")?;
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|_| "Session storage is unavailable")?;
    std::fs::write(path, encrypted).map_err(|_| "Could not save the protected session".into())
}
pub(super) fn clear() -> Result<(), String> {
    if let Some(path) = path() {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err("Could not remove the saved session. Try signing out again.".into());
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn protects_round_trips_and_rejects_modified_data() {
        let secret = b"test-session-not-a-real-credential";
        let mut encrypted = super::protect(secret, false).unwrap();
        assert_ne!(encrypted, secret);
        assert_eq!(super::protect(&encrypted, true).unwrap(), secret);
        let end = encrypted.len() - 1;
        encrypted[end] ^= 1;
        assert!(super::protect(&encrypted, true).is_none());
    }
}
