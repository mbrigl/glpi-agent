// SPDX-License-Identifier: GPL-2.0-only

//! Owned SNMP value type.
//!
//! [`snmp2::Value`] borrows from the session's receive buffer (it holds slices
//! and lazy ASN.1 readers), so it cannot outlive a single request. [`SnmpValue`]
//! is an owned projection of the data-bearing variants that the rest of the
//! crate (MIB modules, the NetInventory task) can keep, compare and serialize.
//! Non-data PDU wrapper variants (`GetRequest`, `Response`, …) convert to
//! `None`.

/// An owned SNMP variable-binding value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnmpValue {
    /// `INTEGER` (and the boolean encoding, widened to `i64`).
    Integer(i64),
    /// `OCTET STRING` — arbitrary bytes, not necessarily UTF-8.
    OctetString(Vec<u8>),
    /// `OBJECT IDENTIFIER`, rendered in dotted-decimal form.
    Oid(String),
    /// `IpAddress` (four octets).
    IpAddress([u8; 4]),
    /// `Counter32`.
    Counter32(u32),
    /// `Gauge32` / `Unsigned32`.
    Unsigned32(u32),
    /// `TimeTicks` (hundredths of a second).
    Timeticks(u32),
    /// `Counter64`.
    Counter64(u64),
    /// `Opaque` wrapper bytes.
    Opaque(Vec<u8>),
    /// `NULL`.
    Null,
    /// SNMPv2 exception: the requested object does not exist.
    NoSuchObject,
    /// SNMPv2 exception: the requested instance does not exist.
    NoSuchInstance,
    /// SNMPv2 exception: end of the MIB view (terminates a walk).
    EndOfMibView,
}

impl SnmpValue {
    /// Projects a borrowed [`snmp2::Value`] into an owned value.
    ///
    /// Returns `None` for the non-data PDU wrapper variants (`GetRequest`,
    /// `Response`, `Trap`, the raw `Sequence`/`Set`/`Constructed` readers), which
    /// never appear as a varbind's value.
    #[must_use]
    pub fn from_snmp2(value: &snmp2::Value<'_>) -> Option<Self> {
        use snmp2::Value as V;
        Some(match value {
            V::Boolean(b) => Self::Integer(i64::from(*b)),
            V::Integer(n) => Self::Integer(*n),
            V::OctetString(bytes) => Self::OctetString(bytes.to_vec()),
            V::ObjectIdentifier(oid) => Self::Oid(oid.to_string()),
            V::IpAddress(octets) => Self::IpAddress(*octets),
            V::Counter32(n) => Self::Counter32(*n),
            V::Unsigned32(n) => Self::Unsigned32(*n),
            V::Timeticks(n) => Self::Timeticks(*n),
            V::Counter64(n) => Self::Counter64(*n),
            V::Opaque(bytes) => Self::Opaque(bytes.to_vec()),
            V::Null => Self::Null,
            V::NoSuchObject => Self::NoSuchObject,
            V::NoSuchInstance => Self::NoSuchInstance,
            V::EndOfMibView => Self::EndOfMibView,
            // PDU/structural variants are not varbind values.
            V::Sequence(_)
            | V::Set(_)
            | V::Constructed(_, _)
            | V::GetRequest(_)
            | V::GetNextRequest(_)
            | V::GetBulkRequest(_)
            | V::Response(_)
            | V::SetRequest(_)
            | V::InformRequest(_)
            | V::Trap(_)
            | V::Report(_) => return None,
        })
    }

    /// Returns the value as a UTF-8 string when it is an `OCTET STRING`,
    /// decoded lossily. Other variants return `None`.
    #[must_use]
    pub fn as_str(&self) -> Option<String> {
        match self {
            Self::OctetString(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
            _ => None,
        }
    }

    /// Returns `true` for the SNMPv2 exception values that terminate a walk or
    /// signal a missing object.
    #[must_use]
    pub fn is_exception(&self) -> bool {
        matches!(
            self,
            Self::NoSuchObject | Self::NoSuchInstance | Self::EndOfMibView
        )
    }
}

#[cfg(test)]
mod tests {
    use super::SnmpValue;

    #[test]
    fn converts_data_variants() {
        assert_eq!(
            SnmpValue::from_snmp2(&snmp2::Value::Integer(42)),
            Some(SnmpValue::Integer(42))
        );
        assert_eq!(
            SnmpValue::from_snmp2(&snmp2::Value::OctetString(b"hello")),
            Some(SnmpValue::OctetString(b"hello".to_vec()))
        );
        assert_eq!(
            SnmpValue::from_snmp2(&snmp2::Value::Counter32(7)),
            Some(SnmpValue::Counter32(7))
        );
        assert_eq!(
            SnmpValue::from_snmp2(&snmp2::Value::Boolean(true)),
            Some(SnmpValue::Integer(1))
        );
    }

    #[test]
    fn converts_object_identifier_to_dotted_string() {
        let oid = snmp2::Oid::from(&[1, 3, 6, 1, 2, 1, 1, 1, 0]).unwrap();
        assert_eq!(
            SnmpValue::from_snmp2(&snmp2::Value::ObjectIdentifier(oid)),
            Some(SnmpValue::Oid("1.3.6.1.2.1.1.1.0".to_owned()))
        );
    }

    #[test]
    fn exceptions_are_flagged() {
        assert!(SnmpValue::from_snmp2(&snmp2::Value::NoSuchObject)
            .unwrap()
            .is_exception());
        assert!(SnmpValue::from_snmp2(&snmp2::Value::EndOfMibView)
            .unwrap()
            .is_exception());
        assert!(!SnmpValue::from_snmp2(&snmp2::Value::Integer(0))
            .unwrap()
            .is_exception());
    }

    #[test]
    fn as_str_decodes_octet_strings_only() {
        assert_eq!(
            SnmpValue::OctetString(b"router-1".to_vec())
                .as_str()
                .as_deref(),
            Some("router-1")
        );
        assert_eq!(SnmpValue::Integer(1).as_str(), None);
    }
}
