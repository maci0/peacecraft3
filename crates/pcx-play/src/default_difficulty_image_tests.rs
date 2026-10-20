use super::*;

const IMAGE_BASE: u32 = 0x40_0000;
const RDATA_RVA: u32 = 0x1000;
const TEXT_RVA: u32 = 0x2000;
const RDATA_RAW: usize = 0x400;
const TEXT_RAW: usize = 0x600;
const RAW_LEN: usize = 0x200;
const PE_AT: usize = 0x80;
const OPTIONAL_LEN: usize = 0xE0;
const SIGNATURE_AT: usize = 0x40;
const REGISTRATION_LEN: usize = 20;
const HANDLER_BASE: usize = 0x100;
const HANDLER_STRIDE: usize = 0x20;
const RDATA_FLAGS: u32 = SCN_MEM_READ;
const TEXT_FLAGS: u32 = SCN_MEM_EXECUTE | SCN_MEM_READ;

/// One registration in `.text`: signature and name pushes, handler push, registrar call.
struct Registration {
    signature_va: u32,
    call: u8,
    handler: Vec<u8>,
}

/// A synthetic PE32 image. Names and layout are invented; only the shapes are real.
struct Spec {
    rdata: Vec<u8>,
    text: Vec<u8>,
    registrations: Vec<Registration>,
    rdata_name: [u8; 8],
    rdata_flags: u32,
    text_flags: u32,
    text_rva: u32,
    text_raw_len: u32,
    count: u16,
}

fn fallback(imm: i32) -> Vec<u8> {
    let mut code = vec![CALL_REL32, 0, 0, 0, 0];
    code.extend_from_slice(&TEST_EAX_EAX);
    code.extend_from_slice(&[JNE_REL8, JNE_SKIPS_FALLBACK, MOV_EAX_IMM32]);
    code.extend_from_slice(&imm.to_le_bytes());
    code.push(RET);
    code
}

fn stub() -> Vec<u8> {
    let mut code = STUB.to_vec();
    code.resize(HANDLER_LEN, 0x90);
    code
}

fn registration(handler: Vec<u8>) -> Registration {
    Registration {
        signature_va: IMAGE_BASE + RDATA_RVA + SIGNATURE_AT as u32,
        call: CALL_REL32,
        handler,
    }
}

impl Spec {
    /// A valid image: primary fallback with the known stub, all names and signatures correct.
    fn valid(imm: i32) -> Self {
        let mut rdata = NAME.to_vec();
        rdata.resize(SIGNATURE_AT, 0);
        rdata.extend_from_slice(SIGNATURE);
        Self {
            rdata,
            text: Vec::new(),
            registrations: vec![registration(fallback(imm)), registration(stub())],
            rdata_name: *RDATA,
            rdata_flags: RDATA_FLAGS,
            text_flags: TEXT_FLAGS,
            text_rva: TEXT_RVA,
            text_raw_len: RAW_LEN as u32,
            count: 2,
        }
    }

    fn with_rdata_signature(mut self, signature: &[u8]) -> Self {
        self.rdata.resize(SIGNATURE_AT, 0);
        self.rdata.extend_from_slice(signature);
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut image = vec![0u8; TEXT_RAW + RAW_LEN];
        image[..2].copy_from_slice(b"MZ");
        image[PE_HEADER_OFFSET..PE_HEADER_OFFSET + 4]
            .copy_from_slice(&(PE_AT as u32).to_le_bytes());
        image[PE_AT..PE_AT + 4].copy_from_slice(PE_SIGNATURE);
        let coff = PE_AT + 4;
        image[coff + 2..coff + 4].copy_from_slice(&self.count.to_le_bytes());
        image[coff + 16..coff + 18].copy_from_slice(&(OPTIONAL_LEN as u16).to_le_bytes());
        let optional = coff + COFF_HEADER_LEN;
        image[optional..optional + 2].copy_from_slice(&PE32_MAGIC.to_le_bytes());
        image[optional + 28..optional + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes());
        let table = optional + OPTIONAL_LEN;
        let sections = [
            (
                self.rdata_name,
                RDATA_RVA,
                RDATA_RAW,
                RAW_LEN as u32,
                self.rdata_flags,
            ),
            (
                *TEXT,
                self.text_rva,
                TEXT_RAW,
                self.text_raw_len,
                self.text_flags,
            ),
        ];
        for (index, (name, rva, raw, raw_len, flags)) in sections.into_iter().enumerate() {
            let at = table + index * SECTION_HEADER_LEN;
            image[at..at + 8].copy_from_slice(&name);
            image[at + 8..at + 12].copy_from_slice(&RAW_LEN.to_le_bytes()[..4]);
            image[at + 12..at + 16].copy_from_slice(&rva.to_le_bytes());
            image[at + 16..at + 20].copy_from_slice(&raw_len.to_le_bytes());
            image[at + 20..at + 24].copy_from_slice(&(raw as u32).to_le_bytes());
            image[at + 36..at + 40].copy_from_slice(&flags.to_le_bytes());
        }
        image[RDATA_RAW..RDATA_RAW + self.rdata.len()].copy_from_slice(&self.rdata);
        let name_va = IMAGE_BASE + RDATA_RVA;
        for (index, registration) in self.registrations.iter().enumerate() {
            let block = TEXT_RAW + index * REGISTRATION_LEN;
            let handler_va = IMAGE_BASE + TEXT_RVA + (HANDLER_BASE + index * HANDLER_STRIDE) as u32;
            image[block] = PUSH_IMM32;
            image[block + 1..block + 5].copy_from_slice(&registration.signature_va.to_le_bytes());
            image[block + 5] = PUSH_IMM32;
            image[block + 6..block + 10].copy_from_slice(&name_va.to_le_bytes());
            image[block + 10] = PUSH_IMM32;
            image[block + 11..block + 15].copy_from_slice(&handler_va.to_le_bytes());
            image[block + 15] = registration.call;
            let at = TEXT_RAW + HANDLER_BASE + index * HANDLER_STRIDE;
            image[at..at + registration.handler.len()].copy_from_slice(&registration.handler);
        }
        for (offset, byte) in self.text.iter().enumerate() {
            image[TEXT_RAW + offset] = *byte;
        }
        image
    }
}

fn expect_err(spec: Spec, want: FallbackError) {
    assert_eq!(native_default_fallback(&spec.bytes()), Err(want));
}

#[test]
fn primary_fallback_and_known_stub_yield_the_fallback_immediate() {
    assert_eq!(native_default_fallback(&Spec::valid(7).bytes()), Ok(7));
    assert_eq!(native_default_fallback(&Spec::valid(-3).bytes()), Ok(-3));
}

#[test]
fn primary_fallback_without_stub_is_accepted() {
    let mut spec = Spec::valid(4);
    spec.registrations.truncate(1);
    assert_eq!(native_default_fallback(&spec.bytes()), Ok(4));
}

#[test]
fn two_competing_fallbacks_refuse() {
    let mut spec = Spec::valid(1);
    spec.registrations.push(registration(fallback(2)));
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn stub_only_refuses() {
    let mut spec = Spec::valid(1);
    spec.registrations.remove(0);
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn two_stubs_refuse() {
    let mut spec = Spec::valid(1);
    spec.registrations.push(registration(stub()));
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn name_must_occur_exactly_once_in_rdata() {
    let mut missing = Spec::valid(1);
    missing.rdata = b"GetDefaultDifficultX\0".to_vec();
    expect_err(missing, FallbackError::Name);
    let mut duplicated = Spec::valid(1);
    duplicated.rdata.resize(0x80, 0);
    duplicated.rdata.extend_from_slice(NAME);
    expect_err(duplicated, FallbackError::Name);
}

#[test]
fn name_outside_rdata_is_not_a_name_anchor() {
    let mut spec = Spec::valid(1);
    spec.rdata = vec![0u8; SIGNATURE_AT];
    spec.text = NAME.to_vec();
    spec.registrations.clear();
    expect_err(spec, FallbackError::Name);
}

#[test]
fn registration_must_carry_the_getter_signature() {
    let spec = Spec::valid(1).with_rdata_signature(b"(Hgamedifficulty;)V\0");
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn registration_call_shape_is_required() {
    let mut spec = Spec::valid(1);
    spec.registrations[0].call = 0x90;
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn handler_must_have_the_fallback_shape() {
    let mut wrong_skip = fallback(1);
    wrong_skip[8] = 5;
    let mut spec = Spec::valid(1);
    spec.registrations[0].handler = wrong_skip;
    expect_err(spec, FallbackError::Registration);
    let mut no_test = fallback(1);
    no_test[5] = 0x90;
    let mut spec = Spec::valid(1);
    spec.registrations[0].handler = no_test;
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn truncated_name_push_in_code_is_not_a_registration() {
    let mut spec = Spec::valid(1);
    spec.registrations.clear();
    let mut spoof = vec![PUSH_IMM32];
    spoof.extend_from_slice(&(IMAGE_BASE + RDATA_RVA).to_le_bytes());
    spec.text = spoof;
    expect_err(spec, FallbackError::Registration);
}

#[test]
fn text_without_execute_or_rdata_without_read_is_refused() {
    let mut no_execute = Spec::valid(1);
    no_execute.text_flags = SCN_MEM_READ;
    expect_err(
        no_execute,
        FallbackError::Sections("required section flags"),
    );
    let mut rdata_exec = Spec::valid(1);
    rdata_exec.rdata_flags = SCN_MEM_READ | SCN_MEM_EXECUTE;
    expect_err(
        rdata_exec,
        FallbackError::Sections("required section flags"),
    );
}

#[test]
fn missing_or_renamed_required_section_is_refused() {
    let mut renamed = Spec::valid(1);
    renamed.rdata_name = *b".data\0\0\0";
    expect_err(renamed, FallbackError::Sections("required section missing"));
}

#[test]
fn section_table_bounds_are_checked_before_reading_headers() {
    for count in [0u16, 97, 0xFFFF] {
        let mut spec = Spec::valid(1);
        spec.count = count;
        assert!(
            matches!(
                native_default_fallback(&spec.bytes()),
                Err(FallbackError::Sections(_))
            ),
            "count {count}"
        );
    }
}

#[test]
fn raw_ranges_outside_the_image_are_refused() {
    let mut spec = Spec::valid(1);
    spec.text_raw_len = 0x10_0000;
    expect_err(spec, FallbackError::Sections("raw data outside image"));
}

#[test]
fn rva_arithmetic_overflow_is_refused() {
    let mut spec = Spec::valid(1);
    spec.text_rva = 0xFFFF_F000;
    expect_err(spec, FallbackError::Sections("address range overflows"));
}

#[test]
fn overlapping_raw_ranges_are_refused() {
    let mut spec = Spec::valid(1);
    spec.text_raw_len = RAW_LEN as u32;
    let mut image = spec.bytes();
    // Point .text's raw data into .rdata's range.
    let table = PE_AT + 4 + COFF_HEADER_LEN + OPTIONAL_LEN;
    let text_header = table + SECTION_HEADER_LEN;
    image[text_header + 20..text_header + 24].copy_from_slice(&(RDATA_RAW as u32).to_le_bytes());
    assert_eq!(
        native_default_fallback(&image),
        Err(FallbackError::Sections("overlapping sections"))
    );
}

#[test]
fn non_pe_and_truncated_images_fail_without_panicking() {
    assert_eq!(
        native_default_fallback(b"not an image"),
        Err(FallbackError::NotPe)
    );
    assert_eq!(native_default_fallback(&[]), Err(FallbackError::NotPe));
    let full = Spec::valid(1).bytes();
    // Bytes after the last handler are zero padding, so shorter prefixes must fail.
    let needed = TEXT_RAW + HANDLER_BASE + HANDLER_STRIDE + HANDLER_LEN;
    for len in 0..needed {
        assert!(
            native_default_fallback(&full[..len]).is_err(),
            "length {len}"
        );
    }
}

/// Source-qualified gate: runs only with the explicitly supplied selected executable.
#[test]
#[ignore = "requires PCX_WAR3_EXE set to the local selected Warcraft III.exe"]
fn selected_executable_yields_the_native_fallback() {
    let path = std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE");
    let image = std::fs::read(path).expect("selected executable");
    // Reviewed Legacy 1.29.2.9231 observation, not a value copied into the derivation.
    assert_eq!(native_default_fallback(&image), Ok(1));
}
