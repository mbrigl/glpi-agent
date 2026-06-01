// SPDX-License-Identifier: GPL-2.0-only

//! IPv4 range expansion for the network scanner.
//!
//! NetDiscovery is driven by a set of address specifications. Each spec is one
//! of three textual forms, all of which [`Ipv4Range::parse`] accepts:
//!
//! * a single address — `192.168.1.10`;
//! * a CIDR block — `192.168.1.0/24` (the network and broadcast addresses are
//!   included, matching the Perl agent's `Net::IP` expansion — callers that
//!   must skip them, such as the Deploy P2P mirror, filter afterwards);
//! * an inclusive range — `192.168.1.10-192.168.1.20`, or with a shorthand
//!   final octet on the right side, `192.168.1.10-20`.
//!
//! A [`Ipv4Range`] holds the inclusive `[start, end]` bounds and yields every
//! address in order via [`Ipv4Range::iter`]. Only IPv4 is supported here; the
//! Perl agent's range scanning is IPv4-only and IPv6 hosts are handed to the
//! scanner as individual addresses elsewhere.

use std::net::Ipv4Addr;

use glpi_core::error::{AgentError, Result};

/// An inclusive range of IPv4 addresses, `[start, end]`.
///
/// Construct one from a textual spec with [`Ipv4Range::parse`] or from explicit
/// bounds with [`Ipv4Range::new`]. Iterate the addresses with [`Ipv4Range::iter`]
/// (or by `&range` directly, via [`IntoIterator`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Range {
    start: u32,
    end: u32,
}

impl Ipv4Range {
    /// Builds a range from explicit inclusive bounds.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Parse`] if `start` is greater than `end`.
    pub fn new(start: Ipv4Addr, end: Ipv4Addr) -> Result<Self> {
        let (start, end) = (u32::from(start), u32::from(end));
        if start > end {
            return Err(AgentError::Parse(format!(
                "IP range start {} is after end {}",
                Ipv4Addr::from(start),
                Ipv4Addr::from(end)
            )));
        }
        Ok(Self { start, end })
    }

    /// Parses one address specification: a single address, a CIDR block, or an
    /// inclusive `start-end` range (the end may be a shorthand final octet).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Parse`] if `spec` is not a well-formed IPv4
    /// address, CIDR block, or range, or if the range bounds are reversed.
    pub fn parse(spec: &str) -> Result<Self> {
        let spec = spec.trim();
        if let Some((addr, prefix)) = spec.split_once('/') {
            return Self::parse_cidr(addr.trim(), prefix.trim());
        }
        if let Some((start, end)) = spec.split_once('-') {
            return Self::parse_range(start.trim(), end.trim());
        }
        let addr = parse_addr(spec)?;
        Self::new(addr, addr)
    }

    fn parse_cidr(addr: &str, prefix: &str) -> Result<Self> {
        let addr = parse_addr(addr)?;
        let prefix: u32 = prefix
            .parse()
            .map_err(|_| AgentError::Parse(format!("invalid CIDR prefix length: {prefix}")))?;
        if prefix > 32 {
            return Err(AgentError::Parse(format!(
                "CIDR prefix length out of range: /{prefix}"
            )));
        }
        // A /0 mask is all-zero; shifting a u32 by 32 is undefined, so special-case it.
        let mask: u32 = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };
        let base = u32::from(addr) & mask;
        let broadcast = base | !mask;
        Ok(Self {
            start: base,
            end: broadcast,
        })
    }

    fn parse_range(start: &str, end: &str) -> Result<Self> {
        let start = parse_addr(start)?;
        // The right-hand side is either a full address or a bare final octet
        // that reuses the first three octets of the start address.
        let end = if let Ok(octet) = end.parse::<u8>() {
            let [a, b, c, _] = start.octets();
            Ipv4Addr::new(a, b, c, octet)
        } else {
            parse_addr(end)?
        };
        Self::new(start, end)
    }

    /// Returns the first address of the range.
    #[must_use]
    pub fn start(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.start)
    }

    /// Returns the last (inclusive) address of the range.
    #[must_use]
    pub fn end(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.end)
    }

    /// Returns the number of addresses in the range (always at least one).
    #[must_use]
    pub fn len(&self) -> u64 {
        u64::from(self.end - self.start) + 1
    }

    /// Always `false`: a range spans at least its single start address.
    ///
    /// Present to satisfy the clippy `len_without_is_empty` lint; a constructed
    /// range can never be empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns `true` if `addr` falls within the inclusive bounds.
    #[must_use]
    pub fn contains(&self, addr: Ipv4Addr) -> bool {
        (self.start..=self.end).contains(&u32::from(addr))
    }

    /// Returns an iterator over every address in the range, in ascending order.
    #[must_use]
    pub fn iter(&self) -> Ipv4RangeIter {
        Ipv4RangeIter {
            next: self.start,
            end: self.end,
            done: false,
        }
    }
}

impl IntoIterator for &Ipv4Range {
    type Item = Ipv4Addr;
    type IntoIter = Ipv4RangeIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the addresses of an [`Ipv4Range`], yielded in ascending order.
///
/// Created by [`Ipv4Range::iter`].
#[derive(Debug, Clone)]
pub struct Ipv4RangeIter {
    next: u32,
    end: u32,
    done: bool,
}

impl Iterator for Ipv4RangeIter {
    type Item = Ipv4Addr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let current = self.next;
        // The range is inclusive of `end`, so detect exhaustion on `end` itself
        // rather than incrementing past it (which would overflow at 255.255.255.255).
        if current == self.end {
            self.done = true;
        } else {
            self.next = current + 1;
        }
        Some(Ipv4Addr::from(current))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.done {
            return (0, Some(0));
        }
        let remaining = u64::from(self.end - self.next) + 1;
        usize::try_from(remaining).map_or((usize::MAX, None), |n| (n, Some(n)))
    }
}

fn parse_addr(s: &str) -> Result<Ipv4Addr> {
    s.parse::<Ipv4Addr>()
        .map_err(|_| AgentError::Parse(format!("invalid IPv4 address: {s}")))
}

#[cfg(test)]
mod tests {
    use super::Ipv4Range;
    use rstest::rstest;
    use std::net::Ipv4Addr;

    #[test]
    fn single_address_is_a_one_element_range() {
        let range = Ipv4Range::parse("192.168.1.10").unwrap();
        assert_eq!(range.start(), Ipv4Addr::new(192, 168, 1, 10));
        assert_eq!(range.end(), Ipv4Addr::new(192, 168, 1, 10));
        assert_eq!(range.len(), 1);
        let addrs: Vec<_> = range.iter().collect();
        assert_eq!(addrs, vec![Ipv4Addr::new(192, 168, 1, 10)]);
    }

    #[test]
    fn cidr_24_includes_network_and_broadcast() {
        let range = Ipv4Range::parse("192.168.1.0/24").unwrap();
        assert_eq!(range.start(), Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(range.end(), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(range.len(), 256);
    }

    #[test]
    fn cidr_normalizes_host_bits_to_the_network() {
        // A host address with a /24 mask still expands to the whole subnet.
        let range = Ipv4Range::parse("192.168.1.37/24").unwrap();
        assert_eq!(range.start(), Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(range.end(), Ipv4Addr::new(192, 168, 1, 255));
    }

    #[test]
    fn cidr_32_is_a_single_host() {
        let range = Ipv4Range::parse("10.0.0.5/32").unwrap();
        assert_eq!(range.len(), 1);
        assert_eq!(range.start(), Ipv4Addr::new(10, 0, 0, 5));
    }

    #[test]
    fn cidr_0_covers_the_entire_space() {
        let range = Ipv4Range::parse("0.0.0.0/0").unwrap();
        assert_eq!(range.start(), Ipv4Addr::UNSPECIFIED);
        assert_eq!(range.end(), Ipv4Addr::BROADCAST);
        assert_eq!(range.len(), 1 << 32);
    }

    #[test]
    fn full_range_is_inclusive() {
        let range = Ipv4Range::parse("192.168.1.10-192.168.1.13").unwrap();
        let addrs: Vec<_> = range.iter().collect();
        assert_eq!(
            addrs,
            vec![
                Ipv4Addr::new(192, 168, 1, 10),
                Ipv4Addr::new(192, 168, 1, 11),
                Ipv4Addr::new(192, 168, 1, 12),
                Ipv4Addr::new(192, 168, 1, 13),
            ]
        );
    }

    #[test]
    fn shorthand_final_octet_range() {
        let range = Ipv4Range::parse("192.168.1.10-20").unwrap();
        assert_eq!(range.start(), Ipv4Addr::new(192, 168, 1, 10));
        assert_eq!(range.end(), Ipv4Addr::new(192, 168, 1, 20));
        assert_eq!(range.len(), 11);
    }

    #[test]
    fn whitespace_is_tolerated() {
        let range = Ipv4Range::parse("  192.168.1.0 / 24 ").unwrap();
        assert_eq!(range.len(), 256);
    }

    #[test]
    fn contains_checks_bounds() {
        let range = Ipv4Range::parse("10.0.0.0/30").unwrap();
        assert!(range.contains(Ipv4Addr::new(10, 0, 0, 0)));
        assert!(range.contains(Ipv4Addr::new(10, 0, 0, 3)));
        assert!(!range.contains(Ipv4Addr::new(10, 0, 0, 4)));
    }

    #[test]
    fn last_address_does_not_overflow() {
        // Exercises the inclusive-end guard at the top of the address space.
        let range = Ipv4Range::parse("255.255.255.254-255.255.255.255").unwrap();
        let addrs: Vec<_> = range.iter().collect();
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[1], Ipv4Addr::BROADCAST);
    }

    #[test]
    fn iterator_size_hint_is_exact() {
        let range = Ipv4Range::parse("192.168.1.0/24").unwrap();
        let iter = range.iter();
        assert_eq!(iter.size_hint(), (256, Some(256)));
        assert_eq!(iter.count(), 256);
    }

    #[rstest]
    #[case("not-an-ip")]
    #[case("192.168.1.300")]
    #[case("192.168.1.0/33")]
    #[case("192.168.1.0/abc")]
    #[case("192.168.1.20-192.168.1.10")]
    #[case("10.0.0.1-10.0.0.0")]
    #[case("")]
    fn malformed_specs_are_rejected(#[case] spec: &str) {
        assert!(Ipv4Range::parse(spec).is_err());
    }
}
