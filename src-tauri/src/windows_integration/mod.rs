//! Integrasi Windows: sisa disk, izin mikrofon, pemutar nada tes.

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
