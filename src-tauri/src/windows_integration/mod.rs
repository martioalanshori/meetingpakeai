//! Integrasi Windows: sisa disk, izin mikrofon, pemutar nada tes, teks clipboard.

pub mod meeting_detect;
pub mod mic_permission;

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_to_caller: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: *mut core::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn GetClipboardData(format: u32) -> *mut core::ffi::c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalLock(mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    fn GlobalUnlock(mem: *mut core::ffi::c_void) -> i32;
    fn GlobalSize(mem: *mut core::ffi::c_void) -> usize;
}

const CF_UNICODETEXT: u32 = 13;
/// Teks clipboard lebih panjang dari ini diabaikan (API key hanya ±56 karakter).
const CLIPBOARD_MAX_CHARS: usize = 4096;

/// Teks di clipboard Windows (`None` jika kosong / bukan teks / terlalu panjang).
pub fn clipboard_text() -> Option<String> {
    // SAFETY: clipboard dibuka-tutup di fungsi ini; pointer hanya dibaca selama GlobalLock,
    // panjang dibatasi GlobalSize dan berhenti di karakter nol.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let mut out = None;
        let handle = GetClipboardData(CF_UNICODETEXT);
        if !handle.is_null() {
            let ptr = GlobalLock(handle) as *const u16;
            if !ptr.is_null() {
                let max = (GlobalSize(handle) / 2).min(CLIPBOARD_MAX_CHARS + 1);
                let len = (0..max).find(|&i| *ptr.add(i) == 0).unwrap_or(max);
                if len <= CLIPBOARD_MAX_CHARS {
                    out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)));
                }
                GlobalUnlock(handle);
            }
        }
        CloseClipboard();
        out
    }
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundW(sound: *const u16, module: *mut core::ffi::c_void, flags: u32) -> i32;
}

/// Byte kosong yang bisa dipakai pengguna di volume tempat `path` berada.
pub fn free_disk_bytes(path: &Path) -> Option<u64> {
    let w = wide(path.as_os_str());
    let mut free = 0u64;
    let mut total = 0u64;
    let mut total_free = 0u64;
    // SAFETY: pointer valid selama panggilan; string diakhiri nol.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, &mut total_free) };
    (ok != 0).then_some(free)
}

const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;
const SND_FILENAME: u32 = 0x0002_0000;

/// Putar file WAV lewat device output default (asinkron).
pub fn play_wav_async(path: &Path) -> bool {
    let w = wide(path.as_os_str());
    // SAFETY: string diakhiri nol; PlaySound menyalin path sebelum kembali.
    unsafe { PlaySoundW(w.as_ptr(), std::ptr::null_mut(), SND_FILENAME | SND_ASYNC | SND_NODEFAULT) != 0 }
}
