use super::super::tests::{fixture, profile};
use super::*;

fn policy() -> QueryPolicy {
    QueryPolicy {
        deadline: Instant::now() + Duration::from_secs(3),
        max_datagrams: 4,
        max_frame_bytes: 1024,
        max_stat_bytes: 1024,
    }
}

fn exchange(max_frame_bytes: usize, foreign_only: bool) -> Result<OwnedFrame, QueryError> {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let target = server.local_addr().unwrap();
    let task = std::thread::spawn(move || {
        let mut request = [0; 64];
        let (length, peer) = server.recv_from(&mut request).unwrap();
        assert_eq!(&request[..length], &profile().search_request());
        if foreign_only {
            let other = UdpSocket::bind("127.0.0.1:0").unwrap();
            other.send_to(&fixture(), peer).unwrap();
        } else {
            server.send_to(&fixture(), peer).unwrap();
        }
    });
    let result = query(
        "127.0.0.1:0".parse().unwrap(),
        target,
        profile(),
        QueryPolicy {
            max_frame_bytes,
            max_datagrams: 1,
            ..policy()
        },
    );
    task.join().unwrap();
    result
}

#[test]
fn actual_udp_query_validates_target_and_complete_advertisement() {
    let frame = exchange(1024, false).unwrap();
    assert_eq!(frame.game_info(profile(), 18).unwrap().map_path(), b"m");
    assert!(matches!(exchange(32, false), Err(QueryError::Oversize)));
    assert!(matches!(
        exchange(1024, true),
        Err(QueryError::DatagramBudget)
    ));
}

#[test]
fn expired_or_invalid_policy_refuses_before_binding() {
    let nonlocal = "192.0.2.1:0".parse().unwrap();
    let target = "127.0.0.1:6112".parse().unwrap();
    assert!(matches!(
        query(
            nonlocal,
            target,
            profile(),
            QueryPolicy {
                deadline: Instant::now(),
                ..policy()
            }
        ),
        Err(QueryError::Deadline)
    ));
    assert!(matches!(
        query(
            nonlocal,
            target,
            profile(),
            QueryPolicy {
                max_datagrams: 0,
                ..policy()
            }
        ),
        Err(QueryError::Policy)
    ));
    assert!(matches!(
        query(
            nonlocal,
            target,
            profile(),
            QueryPolicy {
                max_frame_bytes: usize::MAX,
                ..policy()
            }
        ),
        Err(QueryError::Policy)
    ));
}
