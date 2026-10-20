//! Shared fixtures for the editor-trigger tests: a synthetic TriggerData, a
//! WTG/WCT builder for the version-4 layout, and a synthetic map archive.

// Each test binary uses a subset of these fixtures.
#![allow(dead_code)]

pub const TRIGGER_DATA: &str = "\
[TriggerTypes]
integer=0,1,1,WESTRING_integer
[TriggerTypeDefaults]
integer=0
[TriggerParams]
RealHalf=0,real,0.5,WESTRING_HALF
Num1=0,integer,1,WESTRING_ONE
PlayerOne=0,player,Player(0),WESTRING_P1
OperatorAdd=0,ArithmeticOperator,\"+\",WESTRING_ADD
[TriggerEvents]
TriggerRegisterTimerEventSingle=0,real
[TriggerActions]
DisplayTextToPlayer=0,player,string
SetVariable=0,AnyGlobal,Null
[TriggerCalls]
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
";

pub const BLIZZARD: &[u8] = b"function BlizzardStub takes nothing returns nothing\nendfunction\n";

pub const PRELUDE: &str = "type trigger extends handle
type triggeraction extends handle
type triggercondition extends handle
type event extends handle
type player extends handle
native CreateTrigger takes nothing returns trigger
native DisableTrigger takes trigger t returns nothing
native TriggerAddAction takes trigger t, code c returns triggeraction
native TriggerRegisterTimerEventSingle takes trigger t, real r returns event
native DisplayTextToPlayer takes player p, string s returns nothing
native Player takes integer n returns player
";

pub fn u(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

pub fn s(v: &mut Vec<u8>, text: &str) {
    v.extend_from_slice(text.as_bytes());
    v.push(0);
}

pub fn param(kind: u32, value: &str, sub: Option<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, value);
    u(&mut v, sub.is_some() as u32);
    if let Some(sub) = sub {
        v.extend_from_slice(&sub);
    }
    u(&mut v, 0); // not an array index
    v
}

pub fn nested(name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut sub = Vec::new();
    u(&mut sub, 3);
    s(&mut sub, name);
    u(&mut sub, 1);
    for p in params {
        sub.extend_from_slice(&p);
    }
    param(2, name, Some(sub))
}

pub fn eca(kind: u32, name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, name);
    u(&mut v, 1);
    for p in params {
        v.extend_from_slice(&p);
    }
    v
}

pub fn trigger(name: &str, enabled: bool, ecas: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, name);
    s(&mut v, "");
    u(&mut v, enabled as u32);
    u(&mut v, 0); // custom
    u(&mut v, 0); // initially off
    u(&mut v, 0); // run on init
    u(&mut v, 1); // category
    u(&mut v, ecas.len() as u32);
    for e in ecas {
        v.extend_from_slice(e);
    }
    v
}

/// Count = 5 at init. "Start" registers a timer and, when it fires, adds Num1 to
/// Count and says hi. "Dormant" is disabled and must not be emitted.
pub fn wtg() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"WTG!");
    u(&mut v, 4);
    u(&mut v, 1);
    u(&mut v, 1);
    s(&mut v, "Cat");
    u(&mut v, 1);
    u(&mut v, 1); // one variable
    s(&mut v, "Count");
    s(&mut v, "integer");
    u(&mut v, 1);
    u(&mut v, 0);
    u(&mut v, 1);
    s(&mut v, "5");
    u(&mut v, 2); // two triggers
    let start_ecas = [
        eca(
            0,
            "TriggerRegisterTimerEventSingle",
            vec![param(0, "RealHalf", None)],
        ),
        eca(
            2,
            "SetVariable",
            vec![
                param(1, "Count", None),
                nested(
                    "OperatorInt",
                    vec![
                        param(1, "Count", None),
                        param(0, "OperatorAdd", None),
                        param(0, "Num1", None),
                    ],
                ),
            ],
        ),
        eca(
            2,
            "DisplayTextToPlayer",
            vec![param(0, "PlayerOne", None), param(3, "hi", None)],
        ),
    ];
    v.extend(trigger("Start", true, &start_ecas));
    let dormant = [eca(
        2,
        "DisplayTextToPlayer",
        vec![param(0, "PlayerOne", None), param(3, "never", None)],
    )];
    v.extend(trigger("Dormant", false, &dormant));
    v
}

pub fn wct() -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, 0);
    u(&mut v, 2);
    u(&mut v, 0);
    u(&mut v, 0);
    v
}

pub fn encrypt_mpq_block(data: &mut [u8], key: u32) {
    let table = pcx_mpq::crypt::table();
    let mut seed1 = key;
    let mut seed2 = 0xEEEE_EEEEu32;
    for chunk in data.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xFF) as usize]);
        let plaintext = u32::from_le_bytes(*chunk);
        let ciphertext = plaintext ^ seed1.wrapping_add(seed2);
        chunk.copy_from_slice(&ciphertext.to_le_bytes());
        seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
        seed2 = plaintext
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

pub fn build_synthetic_map(map_name: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    const HASH_ENTRIES: usize = 16;
    let mut payloads = Vec::new();
    let mut block_entries = Vec::new();
    let payload_base_offset = 0x40usize;

    for (_name, data) in files {
        let file_pos = payload_base_offset + payloads.len();
        let file_size = data.len() as u32;
        let compressed_size = file_size;
        let flags: u32 = 0x8000_0000 | 0x0100_0000; // EXISTING | SINGLE_UNIT

        block_entries.push((file_pos as u32, compressed_size, file_size, flags));
        payloads.extend_from_slice(data);
        while payloads.len() % 4 != 0 {
            payloads.push(0);
        }
    }

    let hash_table_pos = (payload_base_offset + payloads.len()) as u32;
    let mut hash_entries = vec![
        pcx_mpq::HashEntry {
            name1: 0xFFFF_FFFF,
            name2: 0xFFFF_FFFF,
            locale: 0,
            platform: 0,
            block_index: 0xFFFF_FFFF,
        };
        HASH_ENTRIES
    ];

    for (i, (name, _)) in files.iter().enumerate() {
        let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
        let mut slot = pcx_mpq::hash::start_slot(name, HASH_ENTRIES);
        while hash_entries[slot].block_index != 0xFFFF_FFFF {
            slot = (slot + 1) % HASH_ENTRIES;
        }
        hash_entries[slot].name1 = name1;
        hash_entries[slot].name2 = name2;
        hash_entries[slot].block_index = i as u32;
    }

    let mut hash_table_raw = Vec::with_capacity(HASH_ENTRIES * 16);
    for entry in &hash_entries {
        hash_table_raw.extend_from_slice(&entry.name1.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.name2.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.locale.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.platform.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.block_index.to_le_bytes());
    }
    encrypt_mpq_block(&mut hash_table_raw, pcx_mpq::hash::hash_table_key());

    let block_table_pos = hash_table_pos + (HASH_ENTRIES as u32 * 16);
    let mut block_table_raw = Vec::with_capacity(block_entries.len() * 16);
    for (pos, comp, size, flags) in &block_entries {
        block_table_raw.extend_from_slice(&pos.to_le_bytes());
        block_table_raw.extend_from_slice(&comp.to_le_bytes());
        block_table_raw.extend_from_slice(&size.to_le_bytes());
        block_table_raw.extend_from_slice(&(*flags).to_le_bytes());
    }
    encrypt_mpq_block(&mut block_table_raw, pcx_mpq::hash::block_table_key());

    let archive_size = block_table_pos + (block_entries.len() as u32 * 16);

    let mut mpq = Vec::new();
    mpq.extend_from_slice(b"MPQ\x1a");
    mpq.extend_from_slice(&0x20u32.to_le_bytes());
    mpq.extend_from_slice(&archive_size.to_le_bytes());
    mpq.extend_from_slice(&0u16.to_le_bytes());
    mpq.push(3u8);
    mpq.push(0u8);
    mpq.extend_from_slice(&hash_table_pos.to_le_bytes());
    mpq.extend_from_slice(&block_table_pos.to_le_bytes());
    mpq.extend_from_slice(&(HASH_ENTRIES as u32).to_le_bytes());
    mpq.extend_from_slice(&(files.len() as u32).to_le_bytes());
    mpq.extend_from_slice(&0u32.to_le_bytes());
    mpq.extend_from_slice(&0u32.to_le_bytes());

    while mpq.len() < payload_base_offset {
        mpq.push(0);
    }
    mpq.extend_from_slice(&payloads);
    mpq.extend_from_slice(&hash_table_raw);
    mpq.extend_from_slice(&block_table_raw);

    assert_eq!(mpq.len(), archive_size as usize);

    let mut map = Vec::new();
    map.extend_from_slice(b"HM3W");
    map.extend_from_slice(&0u32.to_le_bytes());
    map.extend_from_slice(map_name.as_bytes());
    map.push(0);
    map.extend_from_slice(&1u32.to_le_bytes());
    map.extend_from_slice(&1u32.to_le_bytes());
    while map.len() < 0x200 {
        map.push(0);
    }
    map.extend_from_slice(&mpq);
    map
}

/// A variable record: name, type, the unknown word, array flag, initialized flag and value.
pub fn variable(name: &str, ty: &str, initial: Option<&str>) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, name);
    s(&mut v, ty);
    u(&mut v, 1);
    u(&mut v, 0);
    u(&mut v, initial.is_some() as u32);
    s(&mut v, initial.unwrap_or(""));
    v
}

/// A version-4 WTG with the given variable and trigger records.
pub fn wtg_with_variables(variables: &[Vec<u8>], triggers: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"WTG!");
    u(&mut v, 4);
    u(&mut v, 1);
    u(&mut v, 1);
    s(&mut v, "Cat");
    u(&mut v, 1);
    u(&mut v, variables.len() as u32);
    for var in variables {
        v.extend_from_slice(var);
    }
    u(&mut v, triggers.len() as u32);
    for t in triggers {
        v.extend_from_slice(t);
    }
    v
}

/// A WCT with `count` empty blocks, one per trigger.
pub fn empty_wct(count: u32) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, 0);
    u(&mut v, count);
    for _ in 0..count {
        u(&mut v, 0);
    }
    v
}
