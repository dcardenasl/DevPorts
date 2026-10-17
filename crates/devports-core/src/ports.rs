//! Choosing ports.
//!
//! A Principal asks for the port its manifest prefers; a Temporal draws one from
//! a range of high ports. Either way the port must be free *now* and not already
//! promised to another registered instance. Between this check and the process
//! opening the port another program could still take it (ADR-010): the binding
//! rules of `ARQUITECTURA.md`, section 7, are what protect against that.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};

/// Tells whether a port can be used right now.
pub trait PortProbe: Send + Sync {
    /// Whether nothing listens on `port` (on any local address).
    fn is_free(&self, port: u16) -> bool;
}

/// Probes by trying to bind the port on every local address.
///
/// One address is not enough: a dev server bound to `127.0.0.1` does not stop a
/// bind of `0.0.0.0` on every system, and `localhost` may mean `::1`. The port is
/// free only if all four binds succeed (an address family the machine lacks is
/// ignored).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPorts;

impl PortProbe for SystemPorts {
    fn is_free(&self, port: u16) -> bool {
        let addresses = [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        ];
        addresses
            .into_iter()
            .all(|ip| match TcpListener::bind(SocketAddr::new(ip, port)) {
                Ok(_) => true,
                Err(error) => matches!(
                    error.kind(),
                    ErrorKind::AddrNotAvailable | ErrorKind::Unsupported
                ),
            })
    }
}

/// Picks a free port in `range` (both ends included) that is not in `reserved`,
/// scanning from a pseudo-random starting point so concurrent callers spread out.
///
/// `seed` is any number; the same seed with the same state gives the same answer,
/// which keeps tests deterministic.
#[must_use]
pub fn pick_in_range<P: PortProbe + ?Sized>(
    probe: &P,
    range: (u16, u16),
    reserved: &BTreeSet<u16>,
    seed: u64,
) -> Option<u16> {
    let (low, high) = range;
    if low > high {
        return None;
    }
    let span = u64::from(high)
        .saturating_sub(u64::from(low))
        .saturating_add(1);
    let start = seed.checked_rem(span).unwrap_or(0);
    (0..span).find_map(|step| {
        let offset = start.saturating_add(step).checked_rem(span)?;
        let port = u16::try_from(u64::from(low).saturating_add(offset)).ok()?;
        (!reserved.contains(&port) && probe.is_free(port)).then_some(port)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A probe where only the listed ports are busy.
    struct Busy(BTreeSet<u16>);

    impl PortProbe for Busy {
        fn is_free(&self, port: u16) -> bool {
            !self.0.contains(&port)
        }
    }

    fn busy(ports: &[u16]) -> Busy {
        Busy(ports.iter().copied().collect())
    }

    #[test]
    fn the_first_free_port_from_the_starting_point_is_chosen() {
        let probe = busy(&[]);
        assert_eq!(
            pick_in_range(&probe, (5000, 5009), &BTreeSet::new(), 0),
            Some(5000)
        );
        assert_eq!(
            pick_in_range(&probe, (5000, 5009), &BTreeSet::new(), 3),
            Some(5003)
        );
        // The seed wraps around the range.
        assert_eq!(
            pick_in_range(&probe, (5000, 5009), &BTreeSet::new(), 13),
            Some(5003)
        );
    }

    #[test]
    fn busy_and_reserved_ports_are_skipped_wrapping_around() {
        let probe = busy(&[5008]);
        let reserved: BTreeSet<u16> = [5009, 5000].into_iter().collect();
        assert_eq!(
            pick_in_range(&probe, (5000, 5009), &reserved, 8),
            Some(5001)
        );
    }

    #[test]
    fn a_full_range_gives_nothing() {
        let probe = busy(&[7000, 7001]);
        assert_eq!(
            pick_in_range(&probe, (7000, 7001), &BTreeSet::new(), 5),
            None
        );
        let reserved: BTreeSet<u16> = [7000, 7001].into_iter().collect();
        assert_eq!(pick_in_range(&busy(&[]), (7000, 7001), &reserved, 5), None);
    }

    #[test]
    fn an_inverted_range_gives_nothing() {
        assert_eq!(pick_in_range(&busy(&[]), (9, 3), &BTreeSet::new(), 0), None);
    }

    #[test]
    fn the_whole_port_space_does_not_overflow() {
        let probe = busy(&[]);
        assert_eq!(
            pick_in_range(&probe, (0, u16::MAX), &BTreeSet::new(), u64::MAX),
            Some(u16::try_from(u64::MAX % 65_536).unwrap())
        );
    }

    #[test]
    fn the_system_probe_sees_a_port_taken_on_any_local_address() {
        for address in ["127.0.0.1", "0.0.0.0", "[::1]", "[::]"] {
            let Ok(held) = TcpListener::bind(format!("{address}:0")) else {
                continue; // this machine has no such address
            };
            let port = held.local_addr().unwrap().port();
            assert!(!SystemPorts.is_free(port), "held on {address}");
            drop(held);
            assert!(SystemPorts.is_free(port), "released from {address}");
        }
    }
}
