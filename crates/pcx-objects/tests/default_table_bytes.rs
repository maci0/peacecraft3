use pcx_objects::editor::defaults::{DefaultTables, Family, Identity};
use pcx_objects::editor::{EditorReadLimits, SlkId};

const TABLE: &[u8] = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"alias\"\r\nC;X2;Y1;K\"code\"\r\nC;X1;Y2;K\"Q001\"\r\nC;X2;Y2;K\"Qcls\"\r\nE\r\n";
const PROFILE: &[u8] = b"[Q001]\r\nOrder=engage\r\n";
fn identity() -> Identity {
    Identity {
        install: "explicit memory source".into(),
        edition_version: "synthetic edition".into(),
    }
}
fn limits() -> EditorReadLimits {
    EditorReadLimits {
        max_listed_paths: 2,
        max_asset_bytes: 4096,
        max_aggregate_input_bytes: 8192,
    }
}
#[test]
fn supplied_bytes_keep_cells_profiles_and_member_receipts() {
    let tables = DefaultTables::from_bytes(
        identity(),
        &[(SlkId::AbilityData.path(), TABLE)],
        &[("Units\\TestAbilityFunc.txt", PROFILE)],
        limits(),
    )
    .unwrap();
    let ability = tables.object(Family::Ability, "Q001").unwrap();
    let code = ability.cell(SlkId::AbilityData, "code").unwrap();
    assert_eq!(code.text, "Qcls");
    assert_eq!(code.provenance.asset.member, SlkId::AbilityData.path());
    assert_eq!(code.provenance.asset.fnv1a64, pcx_mpq::fnv1a64(TABLE));
    assert_eq!(code.provenance.identity, &identity());
    let order = ability.profile("Order");
    assert_eq!(order.len(), 1);
    assert_eq!(order[0].text, "engage");
    assert_eq!(
        order[0].provenance.asset.member,
        "Units\\TestAbilityFunc.txt"
    );
    assert_eq!(order[0].provenance.asset.fnv1a64, pcx_mpq::fnv1a64(PROFILE));
}
#[test]
fn collection_limits_precede_table_or_profile_decode() {
    for limit in [
        EditorReadLimits {
            max_listed_paths: 1,
            ..limits()
        },
        EditorReadLimits {
            max_asset_bytes: 2,
            ..limits()
        },
        EditorReadLimits {
            max_aggregate_input_bytes: 2,
            ..limits()
        },
    ] {
        let error = DefaultTables::from_bytes(
            identity(),
            &[(SlkId::AbilityData.path(), b"not SYLK")],
            &[("Units\\TestAbilityFunc.txt", b"not a profile")],
            limit,
        )
        .expect_err("collection refuses");
        assert!(
            error.to_string().contains("input budget exceeded"),
            "{error}"
        );
    }
    let duplicate = DefaultTables::from_bytes(
        identity(),
        &[
            (SlkId::AbilityData.path(), TABLE),
            (SlkId::AbilityData.path(), TABLE),
        ],
        &[],
        limits(),
    )
    .expect_err("duplicate input");
    assert!(duplicate.to_string().contains("duplicate supplied member"));
}
