use std::mem::size_of;
use std::sync::OnceLock;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_DWORD, RegCloseKey,
    RegGetValueW, RegOpenKeyExW,
};
use windows::core::{PCWSTR, w};

pub fn allowed() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    *ALLOWED.get_or_init(|| {
        read_disabled(HKEY_LOCAL_MACHINE).or_else(|| read_disabled(HKEY_CURRENT_USER)) != Some(true)
    })
}

fn read_disabled(root: HKEY) -> Option<bool> {
    let mut key = HKEY::default();
    let path = w!("SOFTWARE\\Policies\\Microsoft\\Windows\\CredUI");
    // A successful open yields a handle owned by this function. Close it after the value read.
    unsafe {
        RegOpenKeyExW(root, path, None, KEY_READ, &mut key)
            .ok()
            .ok()?;
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        let status = RegGetValueW(
            key,
            PCWSTR::null(),
            w!("DisablePasswordReveal"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        Some(status.ok().is_ok() && value != 0)
    }
}
