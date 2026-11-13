//! Presentation wall input, independent of simulation clocks and speed controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sample {
    pub milliseconds: u32,
    pub origin: &'static str,
}

#[cfg(any(test, all(target_os = "linux", target_pointer_width = "64")))]
fn boot_word(seconds: u64, nanoseconds: u32) -> Result<u32, String> {
    if nanoseconds >= 1_000_000_000 {
        return Err("audio wall returned invalid nanoseconds".into());
    }
    let milliseconds = u128::from(seconds) * 1000 + u128::from(nanoseconds) / 1_000_000;
    Ok(milliseconds as u32)
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(super) fn sample() -> Result<Sample, String> {
    use std::os::raw::{c_int, c_long};
    #[repr(C)]
    struct Timespec {
        seconds: c_long,
        nanoseconds: c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: c_int, result: *mut Timespec) -> c_int;
    }
    // Linux's CLOCK_BOOTTIME ABI includes suspension in boot-relative time.
    const CLOCK_BOOTTIME: c_int = 7;
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: on this 64-bit Linux ABI Timespec matches struct timespec;
    // clock_gettime writes only the supplied live object and retains no pointer.
    if unsafe { clock_gettime(CLOCK_BOOTTIME, &mut time) } != 0 {
        return Err(format!(
            "audio boot clock: {}",
            std::io::Error::last_os_error()
        ));
    }
    let seconds = u64::try_from(time.seconds).map_err(|_| "audio boot clock is negative")?;
    let nanoseconds =
        u32::try_from(time.nanoseconds).map_err(|_| "audio boot clock nanoseconds")?;
    Ok(Sample {
        milliseconds: boot_word(seconds, nanoseconds)?,
        origin: "system-boot",
    })
}

#[cfg(target_os = "windows")]
pub(super) fn sample() -> Result<Sample, String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount() -> u32;
    }
    // SAFETY: the imported system function has no arguments, retained pointers or failure sentinel.
    Ok(Sample {
        milliseconds: unsafe { GetTickCount() },
        origin: "system-boot",
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn sample() -> Result<Sample, String> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = performance, js_name = now)]
        fn performance_now() -> f64;
    }
    let milliseconds = performance_now();
    if !milliseconds.is_finite() || milliseconds < 0.0 || milliseconds >= u64::MAX as f64 {
        return Err("audio browser monotonic clock is outside its supported range".into());
    }
    // Browser page origin is an explicit adaptation; it is not native boot-seed parity.
    Ok(Sample {
        milliseconds: milliseconds.trunc() as u64 as u32,
        origin: "browser-page",
    })
}

#[cfg(not(any(
    target_arch = "wasm32",
    target_os = "windows",
    all(target_os = "linux", target_pointer_width = "64")
)))]
pub(super) fn sample() -> Result<Sample, String> {
    Err("audio boot-relative clock is unsupported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_wall_word_retains_fractional_units_and_wraps_without_game_time() {
        assert_eq!(boot_word(0, 999_999).unwrap(), 0);
        assert_eq!(boot_word(0, 1_999_999).unwrap(), 1);
        let seconds = u64::from(u32::MAX) / 1000;
        let remainder = (u32::MAX % 1000) * 1_000_000;
        assert_eq!(boot_word(seconds, remainder).unwrap(), u32::MAX);
        assert_eq!(boot_word(seconds, remainder + 1_000_000).unwrap(), 0);
        assert!(boot_word(0, 1_000_000_000).is_err());
    }
    #[test]
    fn audio_wall_sample_declares_its_actual_platform_origin() {
        let value = sample().unwrap();
        if cfg!(target_arch = "wasm32") {
            assert_eq!(value.origin, "browser-page");
        } else {
            assert_eq!(value.origin, "system-boot");
        }
    }
}
