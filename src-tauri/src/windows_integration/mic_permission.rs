//! Status izin mikrofon dari registry ConsentStore (PRD §12.3 `check_mic_permission`).
//! `Value` = "Allow" / "Deny" pada:
//! - `...\ConsentStore\microphone` (akses mikrofon untuk perangkat/pengguna), dan
//! - `...\ConsentStore\microphone\NonPackaged` (izin aplikasi desktop).

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

const CONSENT_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicPermission {
    Allowed,
    Denied,
    Unknown,
}

impl MicPermission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Unknown => "unknown",
        }
    }
}

fn read_value(root: &RegKey, sub: &str) -> Option<String> {
    root.open_subkey(sub).ok()?.get_value::<String, _>("Value").ok()
}

pub fn check() -> MicPermission {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let values = [
        read_value(&hklm, CONSENT_PATH),
        read_value(&hkcu, CONSENT_PATH),
        read_value(&hkcu, &format!(r"{CONSENT_PATH}\NonPackaged")),
    ];
    if values.iter().flatten().any(|v| v.eq_ignore_ascii_case("Deny")) {
        return MicPermission::Denied;
    }
    // Minimal setting pengguna terbaca dan semuanya "Allow".
    let user_known = values[1].is_some() || values[2].is_some();
    if user_known && values.iter().flatten().all(|v| v.eq_ignore_ascii_case("Allow")) {
        MicPermission::Allowed
    } else {
        MicPermission::Unknown
    }
}
