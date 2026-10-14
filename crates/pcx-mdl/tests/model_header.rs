use pcx_mdl::{Error, Mdlx};

fn container() -> Vec<u8> {
    let mut bytes = b"MDLXVERS".to_vec();
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&800_u32.to_le_bytes());
    bytes
}

#[test]
fn missing_model_header_is_a_typed_error_instead_of_a_panic() {
    let bytes = container();
    assert!(matches!(
        Mdlx::parse(&bytes).unwrap().model(&bytes),
        Err(Error::NoModel)
    ));
}

#[test]
fn a_present_short_header_retains_the_truncation_diagnostic() {
    let mut bytes = container();
    bytes.extend_from_slice(b"MODL");
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    assert!(matches!(
        Mdlx::parse(&bytes).unwrap().model(&bytes),
        Err(Error::Truncated {
            what: "MODL extents",
            ..
        })
    ));
}
