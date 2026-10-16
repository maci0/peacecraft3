//! One bounded unicast discovery query to a caller-selected original-protocol host.
//! Broadcast collection, retries after failure and lobby admission are separate.

use super::{DiscoveryError, DiscoveryProfile};
use crate::classic_frame::{FrameError, OwnedFrame, decode_one};
use std::{
    io,
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

/// Explicit input and I/O bounds for one search, without protocol defaults.
#[derive(Debug, Clone, Copy)]
pub struct QueryPolicy {
    /// Absolute deadline for send and every receive, including discarded peers.
    pub deadline: Instant,
    /// Maximum received datagrams, including foreign peers.
    pub max_datagrams: usize,
    /// Largest accepted complete frame, at most the protocol's u16 length range.
    pub max_frame_bytes: usize,
    /// Decoded-stat allocation ceiling.
    pub max_stat_bytes: usize,
}

/// A refused query; no value means the host was joined.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// Invalid byte/count policy, before binding a socket.
    #[error("invalid W3GS query policy")]
    Policy,
    /// Absolute deadline elapsed or socket operation timed out.
    #[error("W3GS query deadline elapsed")]
    Deadline,
    /// Explicit receive work ceiling was exhausted.
    #[error("W3GS query datagram budget exhausted")]
    DatagramBudget,
    /// Datagram exceeds the caller frame ceiling.
    #[error("W3GS query reply exceeds frame limit")]
    Oversize,
    /// Socket error other than timeout or interruption.
    #[error("W3GS query I/O: {0}")]
    Io(#[source] io::Error),
    /// Complete-frame validation failed.
    #[error(transparent)]
    Frame(#[from] FrameError),
    /// Target returned an invalid or incompatible advertisement.
    #[error(transparent)]
    GameInfo(#[from] DiscoveryError),
    /// Fallible receive/retention allocation refused.
    #[error("W3GS query allocation failed")]
    Allocation,
}

fn remaining(deadline: Instant) -> Result<Duration, QueryError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(QueryError::Deadline)
}

fn io_error(error: io::Error) -> QueryError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        QueryError::Deadline
    } else {
        QueryError::Io(error)
    }
}

/// Send one read-only SEARCHGAME and retain the first validated reply from `target`.
///
/// The explicit bind/target addresses are used without DNS. Each receive counts
/// toward the work ceiling, including foreign datagrams and interrupted calls.
/// The search is sent once; failures do not resend it. This does not broadcast, transfer map bytes,
/// change a lobby, or use the project's UDP advertisement protocol.
pub fn query(
    bind: SocketAddr,
    target: SocketAddr,
    profile: DiscoveryProfile,
    policy: QueryPolicy,
) -> Result<OwnedFrame, QueryError> {
    if policy.max_datagrams == 0 || !(4..=usize::from(u16::MAX)).contains(&policy.max_frame_bytes) {
        return Err(QueryError::Policy);
    }
    remaining(policy.deadline)?;
    let mut buffer = Vec::new();
    // One spare byte detects truncation above the admitted frame ceiling.
    buffer
        .try_reserve_exact(policy.max_frame_bytes + 1)
        .map_err(|_| QueryError::Allocation)?;
    buffer.resize(policy.max_frame_bytes + 1, 0);
    let socket = UdpSocket::bind(bind).map_err(io_error)?;
    let request = profile.search_request();
    socket
        .set_write_timeout(Some(remaining(policy.deadline)?))
        .map_err(io_error)?;
    let sent = socket.send_to(&request, target).map_err(io_error)?;
    if sent != request.len() {
        return Err(QueryError::Io(io::Error::new(
            io::ErrorKind::WriteZero,
            "short UDP send",
        )));
    }
    for _ in 0..policy.max_datagrams {
        socket
            .set_read_timeout(Some(remaining(policy.deadline)?))
            .map_err(io_error)?;
        let (bytes, peer) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io_error(error)),
        };
        if peer != target {
            continue;
        }
        if bytes > policy.max_frame_bytes {
            return Err(QueryError::Oversize);
        }
        let frame = decode_one(&buffer[..bytes], policy.max_frame_bytes)?;
        frame.game_info(profile, policy.max_stat_bytes)?;
        buffer.truncate(bytes);
        return Ok(OwnedFrame::from_accepted(buffer));
    }
    Err(QueryError::DatagramBudget)
}

#[cfg(test)]
mod tests;
