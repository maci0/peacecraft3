use pcx_mdl::{Error, Mdlx};

fn read(tag: &[u8; 4], node_size: u32) -> Error {
    let mut bytes = b"MDLXVERS\x04\0\0\0".to_vec();
    bytes.extend(800u32.to_le_bytes());
    bytes.extend(tag);
    bytes.extend(100u32.to_le_bytes());
    bytes.extend(100u32.to_le_bytes());
    bytes.extend(node_size.to_le_bytes());
    bytes.resize(124, 0);
    let m = Mdlx::parse(&bytes).unwrap();
    match tag {
        b"LITE" => m.lights(&bytes).unwrap_err(),
        b"RIBB" => m.ribbons(&bytes).unwrap_err(),
        b"PREM" => m.particles(&bytes).unwrap_err(),
        b"PRE2" => m.particles2(&bytes).unwrap_err(),
        _ => unreachable!(),
    }
}

#[test]
fn invalid_node_lengths_refuse_before_arithmetic_in_every_shared_reader() {
    for tag in [b"LITE", b"RIBB", b"PREM", b"PRE2"] {
        for size in [0, 95, 97, u32::MAX - 3, u32::MAX] {
            assert!(matches!(read(tag, size), Error::EffectRecordOverruns {
                at: 0, size: actual, left: 100, ..
            } if actual == size as usize));
        }
    }
}

#[test]
fn minimum_node_length_reaches_each_specific_body_reader() {
    for tag in [b"LITE", b"RIBB", b"PREM", b"PRE2"] {
        assert!(matches!(read(tag, 96), Error::RecordShape { .. }));
    }
}
