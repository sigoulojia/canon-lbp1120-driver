//! Authentic Canon LBP-1120 Audio Status Feedback
//! Extracted via Reverse Engineering from `CAP3ZWAV.BIN` (Canon Advanced Printing Technology sound library)

use std::ffi::c_void;

#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundA(psz_sound: *const u8, hmod: *mut c_void, fdw_sound: u32) -> i32;
}

const SND_ASYNC: u32 = 0x0001;
const SND_MEMORY: u32 = 0x0004;

pub static SOUND_START: &[u8] = include_bytes!("../assets/sounds/Cap3strt.wav");
pub static SOUND_DONE: &[u8] = include_bytes!("../assets/sounds/Cap3done.wav");
pub static SOUND_ADD_PAPER: &[u8] = include_bytes!("../assets/sounds/Cap3addp.wav");
pub static SOUND_PAPER_LOADED: &[u8] = include_bytes!("../assets/sounds/Cap3come.wav");
pub static SOUND_COVER_OPEN: &[u8] = include_bytes!("../assets/sounds/Cap3covr.wav");
pub static SOUND_JAM: &[u8] = include_bytes!("../assets/sounds/Cap3jam.wav");
pub static SOUND_CARTRIDGE: &[u8] = include_bytes!("../assets/sounds/Cap3cart.wav");
pub static SOUND_ERROR: &[u8] = include_bytes!("../assets/sounds/Cap3err.wav");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonSound {
    PrintStart,
    PrintDone,
    AddPaper,
    PaperLoaded,
    CoverOpen,
    PaperJam,
    NoCartridge,
    Error,
}

pub fn play_sound(sound: CanonSound) {
    let bytes = match sound {
        CanonSound::PrintStart => SOUND_START,
        CanonSound::PrintDone => SOUND_DONE,
        CanonSound::AddPaper => SOUND_ADD_PAPER,
        CanonSound::PaperLoaded => SOUND_PAPER_LOADED,
        CanonSound::CoverOpen => SOUND_COVER_OPEN,
        CanonSound::PaperJam => SOUND_JAM,
        CanonSound::NoCartridge => SOUND_CARTRIDGE,
        CanonSound::Error => SOUND_ERROR,
    };

    unsafe {
        let _ = PlaySoundA(bytes.as_ptr(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC);
    }
}
