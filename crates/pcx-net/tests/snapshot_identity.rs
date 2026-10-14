use pcx_net::rollback::{Correction, Reconciliation, SnapReason};
use pcx_net::{
    Client, CodecError, Datagram, Server, decode, decode_world_payload, encode_world_payload,
};

type Record = (u32, u8, i32, i32, Option<(i32, i32)>);

fn payload(next: u32, records: &[Record]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&next.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());
    for &(id, owner, x, y, target) in records {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(owner);
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
        bytes.push(u8::from(target.is_some()));
        if let Some((x, y)) = target {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
    }
    bytes
}

// Test adapter exercising the existing public decode-then-correct seam.
fn deliver(client: &mut Client, tick: u64, bytes: &[u8]) -> Result<Reconciliation, CodecError> {
    Ok(client.reconcile_detailed(decode_world_payload(tick, bytes)?))
}

#[test]
fn distinct_duplicate_records_report_tick_id_and_both_ordinals() {
    let bytes = payload(8, &[(7, 1, -31, 4, None), (7, 2, 59, 6, Some((9, -11)))]);
    let error = decode_world_payload(47, &bytes).expect_err("ambiguous identity must be refused");
    assert_eq!(
        error.to_string(),
        "snapshot at tick 47 repeats unit ID 7 at record 1; first seen at record 0"
    );
}

#[test]
fn identical_duplicate_records_are_not_two_unique_units() {
    let record = (7, 1, -31, 4, None);
    let bytes = payload(8, &[record, record]);
    let error =
        decode_world_payload(2, &bytes).expect_err("identical records still repeat an identity");
    assert_eq!(
        error.to_string(),
        "snapshot at tick 2 repeats unit ID 7 at record 1; first seen at record 0"
    );
}

#[test]
fn first_nonadjacent_conflict_retains_original_record_index() {
    let bytes = payload(
        10,
        &[
            (7, 1, 0, 0, None),
            (9, 1, 0, 0, None),
            (7, 1, 1, 0, None),
            (9, 1, 1, 0, None),
        ],
    );
    let error = decode_world_payload(6, &bytes).expect_err("nonadjacent repeat must be refused");
    assert_eq!(
        error.to_string(),
        "snapshot at tick 6 repeats unit ID 7 at record 2; first seen at record 0"
    );
}

#[test]
fn checked_delivery_refuses_before_real_client_correction() {
    let initial = decode_world_payload(0, &payload(8, &[(7, 1, 0, 0, None)])).unwrap();
    let mut client = Client::from_world(initial.clone(), 17);
    let before_history = (
        client.history().len(),
        client.history().oldest(),
        client.history().newest(),
    );
    let result = deliver(
        &mut client,
        1,
        &payload(8, &[(7, 1, 64, 0, None), (7, 1, 0, 0, None)]),
    );
    assert_eq!(client.predicted(), &initial);
    assert_eq!(client.authoritative(), &initial);
    assert_eq!(client.consensus_tick(), 0);
    assert_eq!(client.acked_tick(), None);
    assert_eq!(client.pending_inputs(), 0);
    assert_eq!(
        (
            client.history().len(),
            client.history().oldest(),
            client.history().newest()
        ),
        before_history
    );
    assert!(result.is_err());
}

#[test]
fn unique_order_and_raw_id_domain_survive_terminal_counter() {
    let records = [
        (7, 255, -31, 4, None),
        (3, 0, 59, 6, Some((17, -21))),
        (u32::MAX, 2, i32::MIN, i32::MAX, None),
        (0, 8, 0, 0, None),
    ];
    let bytes = payload(u32::MAX, &records);
    let world = decode_world_payload(13, &bytes).unwrap();
    assert_eq!(world.next_unit_id, u32::MAX);
    assert_eq!(
        world.units.iter().map(|unit| unit.id).collect::<Vec<_>>(),
        vec![7, 3, u32::MAX, 0]
    );
    assert_eq!(encode_world_payload(&world), bytes);
    assert_eq!(world.unit(u32::MAX).unwrap().x, i32::MIN);
    assert_eq!(world.unit(0).unwrap().owner, 8);
    let empty = payload(u32::MAX, &[]);
    assert_eq!(
        encode_world_payload(&decode_world_payload(14, &empty).unwrap()),
        empty
    );
}

#[test]
fn unique_decoded_snapshot_reaches_the_actual_client_unchanged() {
    let initial = decode_world_payload(0, &payload(8, &[(7, 1, 0, 0, None)])).unwrap();
    let mut client = Client::from_world(initial, 17);
    let bytes = payload(
        u32::MAX,
        &[(7, 1, 64, 0, Some((90, -3))), (0, 2, -9, 11, None)],
    );
    let report = deliver(&mut client, 1, &bytes).unwrap();
    assert_eq!(report.divergence, 64);
    assert_eq!(
        report.correction,
        Correction::Snapped {
            tick: 1,
            reason: SnapReason::Unconfirmed
        }
    );
    assert_eq!(client.predicted(), &report.snapshot);
    assert_eq!(client.authoritative(), &report.snapshot);
    assert_eq!(client.consensus_tick(), 1);
    assert_eq!(encode_world_payload(client.predicted()), bytes);
}

#[test]
fn existing_framing_errors_remain_contextual() {
    let mut bytes = payload(8, &[(7, 1, 0, 0, None)]);
    bytes.push(255);
    assert!(matches!(
        decode_world_payload(0, &bytes),
        Err(CodecError::TrailingBytes { remaining: 1 })
    ));
    let short = &bytes[..10];
    assert!(matches!(
        decode_world_payload(0, short),
        Err(CodecError::PayloadTooShort { kind: 3, .. })
    ));
}

#[test]
fn truncated_duplicate_record_reports_framing_before_identity() {
    let mut bytes = payload(8, &[(7, 1, 0, 0, None), (7, 1, 1, 0, Some((9, 11)))]);
    let full_length = bytes.len();
    bytes.pop();
    assert!(
        matches!(decode_world_payload(4, &bytes), Err(CodecError::PayloadTooShort { kind: 3, need, got }) if need == full_length && got == full_length - 1)
    );
}

#[test]
fn actual_server_session_snapshot_preserves_unique_stored_order() {
    let bytes = payload(u32::MAX, &[(9, 1, 4, 5, None), (0, 2, 6, 7, Some((8, 9)))]);
    let world = decode_world_payload(3, &bytes).unwrap();
    let server = Server::new(world.clone(), 71);
    assert_eq!(encode_world_payload(server.world()), bytes);
    let session = pcx_net::session::Session::new(world.clone(), 71);
    let Datagram::Snapshot {
        tick,
        payload: encoded,
    } = decode(&session.snapshot()).unwrap()
    else {
        panic!("session must encode a snapshot")
    };
    assert_eq!(tick, 3);
    assert_eq!(encoded, bytes);
    assert_eq!(decode_world_payload(tick, &encoded).unwrap(), world);
}

#[test]
fn duplicate_refusal_is_typed_including_raw_zero_and_maximum_ids() {
    for id in [0, 7, u32::MAX] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        for unit_id in [id, id ^ 1, id] {
            bytes.extend_from_slice(&unit_id.to_le_bytes());
            bytes.push(1);
            bytes.extend_from_slice(&0i32.to_le_bytes());
            bytes.extend_from_slice(&0i32.to_le_bytes());
            bytes.push(0);
        }
        assert!(
            matches!(decode_world_payload(71, &bytes), Err(CodecError::DuplicateUnitId { tick: 71, id: refused, first_record: 0, record: 2 }) if refused == id)
        );
    }
}

#[test]
fn complete_framing_precedes_identity_metadata_on_multi_error_frame() {
    let mut bytes = vec![];
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&3u32.to_le_bytes());
    for (id, target) in [(7u32, false), (7, false), (9, true)] {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.push(u8::from(target));
        if target {
            bytes.extend_from_slice(&0i32.to_le_bytes());
            bytes.extend_from_slice(&0i32.to_le_bytes());
        }
    }
    bytes.pop();
    assert_eq!(bytes.len(), 57);
    assert!(matches!(
        decode_world_payload(71, &bytes),
        Err(CodecError::PayloadTooShort {
            kind: 3,
            need: 58,
            got: 57
        })
    ));
}
