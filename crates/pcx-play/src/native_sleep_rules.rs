//! Source joins for map and AI sleep; their clocks remain independently driven.
use super::{call, shape, word};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Game,
    Real,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub script: Domain,
    pub ai: Domain,
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let script_native = registration(read, 0x49cb25, b"TriggerSleepAction\0")?;
        let ai_native = registration(read, 0xab60d7, b"Sleep\0")?;
        let script = read(script_native, 0x12, true)?;
        shape(&script, 3, &[0x6a, 0, 0xff, 0x75, 8])?;
        call(&script, script_native, 8, 0x8cd210)?;
        let setter = read(0x8cd210, 0x42, true)?;
        shape(
            &setter,
            0x2b,
            &[0x8b, 0x45, 8, 0xc7, 0x41, 0x34, 1, 0, 0, 0],
        )?;
        shape(
            &setter,
            0x35,
            &[0x8b, 0, 0x89, 0x41, 0x40, 0x8b, 0x45, 0xc, 0x89, 0x41, 0x38],
        )?;
        let ai = read(ai_native, 0x45, true)?;
        call(&ai, ai_native, 0x2d, 0xac3e20)?;
        call(&ai, ai_native, 0x3e, 0xac3e20)?;
        let ai_sleep = read(0xac3e20, 0x59, true)?;
        shape(&ai_sleep, 4, &[0x6a, 0, 0x68])?;
        call(&ai_sleep, 0xac3e20, 0xd, 0x8cd210)?;
        shape(&ai_sleep, 0x12, &[0x8b, 0x86, 0x9c, 2, 0, 0])?;
        shape(
            &ai_sleep,
            0x49,
            &[
                0x6a, 1, 0xff, 0x75, 8, 0x8d, 0x48, 0x24, 0x8b, 1, 0xff, 0x10,
            ],
        )?;
        // Both embedded timers must dispatch the extended start routine.
        let script_ctor = read(0x4f9610, 0x9f, true)?;
        let ai_ctor = read(0xad33a0, 0xe7, true)?;
        shape(&script_ctor, 0x6a, &[0x8d, 0x4f, 0x34])?;
        shape(&script_ctor, 0x93, &[0xc7, 1])?;
        shape(&ai_ctor, 0x73, &[0x8d, 0x4f, 0x44])?;
        shape(&ai_ctor, 0xe1, &[0xc7, 1])?;
        let vtable = word(&script_ctor, 0x95)?;
        if word(&ai_ctor, 0xe3)? != vtable || word(&read(vtable, 12, false)?, 8)? != 0x7fb820 {
            return Err("native sleep timer family differs".into());
        }
        let dispatch = read(0x7fb820, 0x79, true)?;
        shape(&dispatch, 0x58, &[0x8b, 0x45, 0x18, 0x85, 0xc0])?;
        shape(
            &dispatch,
            0x6d,
            &[
                0x85, 0xc0, 0x74, 5, 0x8b, 0x46, 0x48, 0xeb, 3, 0x8b, 0x46, 0x44,
            ],
        )?;
        let script_start = read(0x4fbdd0, 0x6a, true)?;
        call(&script_start, 0x4fbdd0, 0x4b, 0x8cc3d0)?;
        shape(&script_start, 0x56, &[0x8d, 0x4e, 0x34, 0x6a])?;
        shape(&script_start, 0x5b, &[0x6a, 0, 0x56, 0x6a, 0, 0x50])?;
        call(&script_start, 0x4fbdd0, 0x61, 0x7fbf80)?;
        let ai_run = read(0xab5703, 0x22, true)?;
        shape(
            &ai_run,
            0,
            &[0x83, 0xe8, 2, 0x74, 0x11, 0x83, 0xe8, 1, 0x75, 0x18],
        )?;
        shape(
            &ai_run,
            0xa,
            &[0x8b, 0x47, 0x28, 0x89, 0x45, 0xec, 0x8d, 0x45, 0xec, 0x50],
        )?;
        call(&ai_run, 0xab5703, 0x1d, 0xad3560)?;
        let ai_start = read(0xad3560, 0x1c, true)?;
        shape(&ai_start, 3, &[0x6a])?;
        shape(&ai_start, 5, &[0x6a, 0, 0x51, 0x68])?;
        shape(&ai_start, 0xd, &[0xff, 0x75, 8, 0x83, 0xc1, 0x44])?;
        call(&ai_start, 0xad3560, 0x13, 0x7fbf80)?;
        Ok(Self {
            script: domain(
                *script_start
                    .get(0x5a)
                    .ok_or("native map sleep selector missing")?,
            )?,
            ai: domain(*ai_start.get(4).ok_or("native AI sleep selector missing")?)?,
        })
    }
}

fn registration(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    name: &[u8],
) -> Result<u32, String> {
    let b = read(va, 20, true)?;
    for at in [0, 5, 10] {
        shape(&b, at, &[0x68])?;
    }
    call(&b, va, 15, 0x8ccc80)?;
    if read(word(&b, 6)?, name.len(), false)? != name {
        return Err("native sleep registration name changed".into());
    }
    word(&b, 11)
}

fn domain(selector: u8) -> Result<Domain, String> {
    match selector {
        0 => Ok(Domain::Game),
        1 => Ok(Domain::Real),
        _ => Err("native sleep selector unsupported".into()),
    }
}
