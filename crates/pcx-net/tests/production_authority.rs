//! The production authority on a real admitted map, driven through the session trait without
//! sockets. Ignored by default: the gate needs the selected install and map.
//!
//! PCX_STARTUP_INSTALL=<install dir> PCX_STARTUP_MAP=<map file> PCX_STARTUP_ADDON=roc|tft \
//!   cargo test -p pcx-net --features session-server --test production_authority -- --ignored
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/production.rs"]
mod production;

use pcx_net::Peer;
use pcx_net::production::ProductionAuthority;
use pcx_net::session::{SessionAuthority, SessionError};
use pcx_sim::{Command, FX_ONE, Unit, World};
use production::admit;

/// The world a snapshot payload describes.
fn snapshot_world(bytes: &[u8]) -> World {
    match pcx_net::decode(bytes).expect("snapshot datagram") {
        pcx_net::Datagram::Snapshot { tick, payload } => {
            pcx_net::decode_world_payload(tick, &payload).expect("snapshot payload")
        }
        other => panic!("not a snapshot: {other:?}"),
    }
}

fn owned_unit(world: &World, owner: u8) -> Unit {
    world
        .units
        .iter()
        .find(|unit| unit.owner == owner)
        .cloned()
        .unwrap_or_else(|| panic!("owner {owner} has no exported unit"))
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP and PCX_STARTUP_ADDON selected local files"]
fn roster_orders_and_refusals_on_the_admitted_map() {
    let mut authority = admit(1);
    let roster: Vec<u8> = authority.roster().collect();
    assert_eq!(roster.len(), 1, "this gate expects one human slot");
    let slot = roster[0];
    let outside = (0..16u8)
        .find(|candidate| !roster.contains(candidate))
        .expect("a non-roster slot");

    // Admission: a non-roster slot is refused; the roster slot joins once.
    assert!(matches!(
        authority.join(Peer(1), outside),
        Err(SessionError::NotAdmitted { player }) if player == outside
    ));
    authority.join(Peer(1), slot).expect("roster slot joins");
    assert!(
        matches!(authority.join(Peer(2), slot), Err(SessionError::Identity)),
        "a second peer cannot take a bound slot"
    );

    // Gating: at tick 0 the script has user control off, so a move batch is refused with that
    // reason and is not applied. Campaign maps can stay paused past the window this gate runs
    // (see docs/re-notes/native-session-peers.md), so acceptance is not asserted here.
    let start = snapshot_world(&authority.snapshot().expect("snapshot"));
    let unit = owned_unit(&start, slot);
    let target = (unit.x + 64 * FX_ONE, unit.y);
    let refusals_before_move = refusal_total(&authority);
    let tick = authority.tick();
    authority
        .receive(
            Peer(1),
            &batch(
                tick,
                slot,
                &[Command::Move {
                    x: target.0,
                    y: target.1,
                }],
            ),
        )
        .expect("the packet is well formed");
    authority.advance().expect("advance after a refusal");
    assert_eq!(
        refusal_total(&authority),
        refusals_before_move + 1,
        "the move was not refused"
    );
    let refusal = authority
        .refusals()
        .entries()
        .last()
        .expect("refusal")
        .clone();
    assert_eq!(refusal.reason, "user control is disabled by the script");
    assert_eq!(refusal.tick, tick);

    // Wrong owner: a packet that names another player's slot is refused as an identity error,
    // and it is not recorded as a refusal of the batch.
    let refusals_before_wrong_owner = refusal_total(&authority);
    let tick = authority.tick();
    assert!(matches!(
        authority.receive(Peer(1), &batch(tick, outside, &[Command::Nop])),
        Err(SessionError::Identity)
    ));
    assert_eq!(refusal_total(&authority), refusals_before_wrong_owner);

    // Selection and stop are refused and recorded; the refusal does not stop the authority.
    let now = authority.tick();
    let refusals_before_stop = refusal_total(&authority);
    authority
        .receive(Peer(1), &batch(now, slot, &[Command::Stop]))
        .expect("the packet is well formed");
    authority.advance().expect("advance after a refusal");
    assert_eq!(
        refusal_total(&authority),
        refusals_before_stop + 1,
        "the stop batch was not refused"
    );

    // Stale, duplicate and too-far batches are refused at receipt.
    assert!(matches!(
        authority.receive(Peer(1), &batch(0, slot, &[Command::Nop])),
        Err(SessionError::Tick(text)) if text.contains("already run")
    ));
    let future = authority.tick() + 5;
    authority
        .receive(Peer(1), &batch(future, slot, &[Command::Nop]))
        .expect("first batch");
    assert!(matches!(
        authority.receive(Peer(1), &batch(future, slot, &[Command::Nop])),
        Err(SessionError::Duplicate)
    ));
    assert!(matches!(
        authority.receive(
            Peer(1),
            &batch(authority.tick() + 1_000, slot, &[Command::Nop])
        ),
        Err(SessionError::Tick(_))
    ));

    // Resync returns current authority and never rewinds the clock.
    let now = authority.tick();
    let resynced = authority
        .receive(
            Peer(1),
            &pcx_net::encode(&pcx_net::Datagram::Resync { from_tick: 0 }),
        )
        .expect("resync")
        .expect("resync replies");
    assert!(snapshot_world(&resynced).tick >= now);

    // Reconnect: the slot is free after leave and rejoins for the same player.
    authority.leave(Peer(1));
    authority
        .join(Peer(3), slot)
        .expect("reconnect reclaims the slot");
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP and PCX_STARTUP_ADDON selected local files"]
fn admission_identity_covers_the_seed() {
    let first = admit(1);
    let again = admit(1);
    let other = admit(2);
    assert_eq!(
        first.identity(),
        again.identity(),
        "identical inputs must agree"
    );
    assert_ne!(
        first.identity(),
        other.identity(),
        "a different seed must change the identity"
    );
}

/// A peer batch for `tick`, as the wire carries it.
fn batch(tick: u64, player: u8, commands: &[Command]) -> Vec<u8> {
    pcx_net::encode(&pcx_net::Datagram::Input {
        tick,
        player,
        payload: pcx_net::encode_input_payload(commands),
    })
}

/// Every refusal so far, including those the bounded log has dropped.
fn refusal_total(authority: &ProductionAuthority) -> u64 {
    authority.refusals().dropped() + authority.refusals().entries().count() as u64
}
