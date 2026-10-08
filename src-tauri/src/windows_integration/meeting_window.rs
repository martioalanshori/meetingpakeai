//! Judul jendela aplikasi meeting (langkah 46, feedback3 A0.4 & A2): mengenali Google Meet di browser dan
//! mengambil nama meeting dari jendela Teams / Meet sebagai judul awal.

use std::path::Path;

type Hwnd = *mut core::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
    fn QueryFullProcessImageNameW(process: *mut core::ffi::c_void, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

unsafe extern "system" fn collect(hwnd: Hwnd, lparam: isize) -> i32 {
    // SAFETY: `lparam` adalah pointer ke Vec milik `visible_windows` yang hidup selama EnumWindows berjalan.
    let out = unsafe { &mut *(lparam as *mut Vec<(u32, String)>) };
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 || len > 1024 {
            return 1;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if n <= 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        out.push((pid, String::from_utf16_lossy(&buf[..n as usize])));
    }
    1
}

fn exe_name(pid: u32) -> Option<String> {
    // SAFETY: handle proses dibuka hanya untuk membaca nama image lalu ditutup.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        Path::new(&path).file_name().map(|n| n.to_string_lossy().to_lowercase())
    }
}

/// Jendela top-level yang terlihat: (nama exe lowercase, judul).
fn visible_windows() -> Vec<(String, String)> {
    let mut raw: Vec<(u32, String)> = Vec::new();
    // SAFETY: callback hanya menulis ke `raw` selama panggilan ini.
    unsafe {
        EnumWindows(collect, &mut raw as *mut _ as isize);
    }
    let mut names: std::collections::HashMap<u32, Option<String>> = std::collections::HashMap::new();
    raw.into_iter()
        .filter_map(|(pid, title)| {
            let exe = names.entry(pid).or_insert_with(|| exe_name(pid)).clone()?;
            Some((exe, title))
        })
        .collect()
}

/// Pola judul tab Google Meet bawaan (bisa diganti `meeting_detection.meet_title_patterns` di providers.json).
pub fn default_meet_patterns() -> Vec<String> {
    ["Meet - ", "Meet – ", "Meet: ", "meet.google.com"].iter().map(|s| s.to_string()).collect()
}

const BROWSERS: [&str; 5] = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"];
const TEAMS: [&str; 2] = ["ms-teams.exe", "teams.exe"];

/// Judul tab browser tanpa akhiran nama browser/profil ("… - Google Chrome", "… - Pribadi - Microsoft Edge").
fn strip_browser_suffix(title: &str) -> &str {
    let mut t = title;
    loop {
        let Some(i) = t.rfind(" - ") else { return t };
        let tail = t[i + 3..].to_lowercase();
        let browserish = ["chrome", "edge", "firefox", "brave", "opera", "profil", "profile", "pribadi", "personal", "work"]
            .iter()
            .any(|k| tail.contains(k));
        if !browserish {
            return t;
        }
        t = &t[..i];
    }
}

/// Kode rapat Meet ("abc-defg-hij") bukan nama meeting.
fn is_meet_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()))
}

/// Judul tab Google Meet di salah satu jendela browser (`patterns` dari providers.json).
fn meet_tab(windows: &[(String, String)], patterns: &[String]) -> Option<String> {
    windows.iter().filter(|(exe, _)| BROWSERS.contains(&exe.as_str())).find_map(|(_, title)| {
        let tab = strip_browser_suffix(title);
        patterns.iter().any(|p| !p.is_empty() && tab.contains(p.as_str())).then(|| tab.to_string())
    })
}

/// Google Meet sedang terbuka di browser (tab aktif sebuah jendela).
pub fn meet_open(patterns: &[String]) -> bool {
    meet_tab(&visible_windows(), patterns).is_some()
}

/// Nama meeting dari jendela aplikasi meeting, mis. Teams "Sync Mingguan | Microsoft Teams" atau
/// Meet "Meet – Sync Mingguan". Judul umum (Zoom Meeting, kode Meet, menu Teams) → `None`.
pub fn meeting_title(patterns: &[String]) -> Option<(String, String)> {
    let windows = visible_windows();
    if let Some(tab) = meet_tab(&windows, patterns) {
        let name = tab
            .trim_start_matches("Meet")
            .trim_start_matches([' ', '-', '–', ':'])
            .trim();
        if !name.is_empty() && !is_meet_code(name) && !name.contains("meet.google.com") {
            return Some(("meet".into(), name.to_string()));
        }
    }
    let generic = [
        "microsoft teams", "chat", "obrolan", "kalender", "calendar", "aktivitas", "activity", "teams", "tim",
        "rapat", "meeting", "meet", "panggilan", "calls", "files", "file",
    ];
    windows.iter().filter(|(exe, _)| TEAMS.contains(&exe.as_str())).find_map(|(_, title)| {
        let name = title.split(" | ").next().unwrap_or(title).trim();
        let lower = name.to_lowercase();
        (name.chars().count() >= 4 && !generic.contains(&lower.as_str()) && !lower.starts_with("microsoft teams"))
            .then(|| ("teams".to_string(), name.to_string()))
    })
}
