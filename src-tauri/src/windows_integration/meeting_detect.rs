//! Aplikasi yang sedang memakai mikrofon menurut registry ConsentStore (PRD §14.8).
//! Subkey `microphone\NonPackaged\<path exe, '\' → '#'>` dan subkey aplikasi packaged langsung di bawah
//! `microphone`; aplikasi sedang memakai mic jika `LastUsedTimeStop = 0`.

use std::collections::{BTreeMap, BTreeSet};

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const MIC_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

fn in_use(key: &RegKey) -> bool {
    key.get_value::<u64, _>("LastUsedTimeStop").is_ok_and(|v| v == 0)
        && key.get_value::<u64, _>("LastUsedTimeStart").is_ok_and(|v| v != 0)
}

/// Jenis aplikasi meeting untuk nama subkey registry; `None` jika bukan aplikasi meeting.
/// `apps`: nama exe / prefiks paket (lowercase) → jenis (`zoom`, `teams`, `browser`, ...).
fn classify(subkey: &str, packaged: bool, apps: &BTreeMap<String, String>, own_exe: &str) -> Option<String> {
    let name = if packaged {
        subkey.to_lowercase()
    } else {
        subkey.rsplit('#').next().unwrap_or(subkey).to_lowercase()
    };
    if !packaged && name == own_exe {
        return None;
    }
    if packaged {
        // Paket: "MSTeams_8wekyb3d8bbwe" → cocokkan prefiks sebelum "_".
        let family = name.split('_').next().unwrap_or(&name);
        apps.get(family).cloned()
    } else {
        apps.get(&name).cloned()
    }
}

/// Aplikasi meeting yang sedang memakai mic: kunci unik (subkey) → jenis aplikasi.
pub fn active_meeting_apps(apps: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let own_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mut out = BTreeMap::new();
    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(MIC_PATH) else { return out };

    if let Ok(np) = root.open_subkey("NonPackaged") {
        for sub in np.enum_keys().flatten() {
            if let Some(kind) = classify(&sub, false, apps, &own_exe) {
                if np.open_subkey(&sub).is_ok_and(|k| in_use(&k)) {
                    out.insert(sub, kind);
                }
            }
        }
    }
    let skip: BTreeSet<&str> = ["NonPackaged"].into();
    for sub in root.enum_keys().flatten() {
        if skip.contains(sub.as_str()) {
            continue;
        }
        if let Some(kind) = classify(&sub, true, apps, &own_exe) {
            if root.open_subkey(&sub).is_ok_and(|k| in_use(&k)) {
                out.insert(sub, kind);
            }
        }
    }
    out
}
