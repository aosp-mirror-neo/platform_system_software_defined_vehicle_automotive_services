// Copyright (C) 2026 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Diagnostics Compliance Identifiers Library
//!
//! A shared enum to that provides the mapping between DIDs and DataItem names.
//! The mapping is specified in the SDV Diagnostics requirements.

use std::ops::RangeInclusive;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

/// The range of Google-reserved Data Identifiers (DIDs) for compliance testing.
pub const GOOGLE_RESERVED_RANGE: RangeInclusive<u16> = 0xF0F0..=0xF0FF;

/// Diagnostics Compliance Identifiers for DID and DataItem name mapping.
#[derive(AsRefStr, Debug, PartialEq, Eq, Clone, Copy, Display, EnumString, EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum DiagComplianceId {
    /// Mapping for DID 0xF0F0
    #[strum(serialize = "reserved_0")]
    Reserved0,
    /// Mapping for DID 0xF0F1
    #[strum(serialize = "reserved_1")]
    Reserved1,
    /// Mapping for DID 0xF0F2
    #[strum(serialize = "reserved_2")]
    Reserved2,
    /// Mapping for DID 0xF0F3 - 0xF0FF
    #[strum(serialize = "default")]
    Default,
}

impl DiagComplianceId {
    /// Map enum to its u16 byte value, `Default` is mapped to 0xF0FF
    pub fn to_u16(&self) -> u16 {
        match self {
            Self::Reserved0 => 0xF0F0,
            Self::Reserved1 => 0xF0F1,
            Self::Reserved2 => 0xF0F2,
            Self::Default => 0xF0FF,
        }
    }

    /// Create enum from supported u16 values or None if not supported
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0xF0F0 => Some(Self::Reserved0),
            0xF0F1 => Some(Self::Reserved1),
            0xF0F2 => Some(Self::Reserved2),
            0xF0F3..=0xF0FF => Some(Self::Default),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use strum::IntoEnumIterator;

    // --- Reserved Explicit Mappings ---

    #[test]
    fn reserved0_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F0).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved0);
        assert_eq!(id.to_u16(), 0xF0F0);
        assert_eq!(id.to_string(), "reserved_0");
        assert_eq!(DiagComplianceId::from_str("reserved_0").unwrap(), id);
    }

    #[test]
    fn reserved1_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F1).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved1);
        assert_eq!(id.to_u16(), 0xF0F1);
        assert_eq!(id.to_string(), "reserved_1");
        assert_eq!(DiagComplianceId::from_str("reserved_1").unwrap(), id);
    }

    #[test]
    fn reserved2_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F2).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved2);
        assert_eq!(id.to_u16(), 0xF0F2);
        assert_eq!(id.to_string(), "reserved_2");
        assert_eq!(DiagComplianceId::from_str("reserved_2").unwrap(), id);
    }

    // --- Default Range Mappings ---

    #[test]
    fn default_variant_string_mapping_is_correct() {
        let id = DiagComplianceId::Default;
        assert_eq!(DiagComplianceId::from_str("default").unwrap(), id);
        assert_eq!(id.to_string(), "default");
    }

    #[test]
    fn default_variant_to_u16_mapping_is_correct() {
        // Default enum should always map back to 0xF0FF
        assert_eq!(DiagComplianceId::Default.to_u16(), 0xF0FF);
    }

    #[test]
    fn from_u16_with_default_range_maps_to_default_variant() {
        for did in 0xF0F3..=0xF0FF {
            let id = DiagComplianceId::from_u16(did).unwrap_or_else(|| {
                panic!("DID {did:#X} should be supported in `DiagComplianceId`")
            });
            assert_eq!(id, DiagComplianceId::Default, "DID {did:#X} should map to Default");
        }
    }

    // --- Google Reserved Range ---

    #[test]
    fn google_reserved_range_contains_only_valid_values() {
        for did in GOOGLE_RESERVED_RANGE {
            assert!(
                DiagComplianceId::from_u16(did).is_some(),
                "DID {did:#X} should be supported in `DiagComplianceId`"
            );
        }
    }

    #[test]
    fn all_variants_exist_in_google_reserved_range() {
        for id in DiagComplianceId::iter() {
            let did = id.to_u16();
            assert!(
                GOOGLE_RESERVED_RANGE.contains(&did),
                "DID {did:#X} should be contained in `GOOGLE_RESERVED_RANGE`"
            );
        }
    }

    // --- Negative Tests ---

    #[test]
    fn from_u16_returns_none_for_unsupported_values() {
        // Test lower bound - 1
        assert!(DiagComplianceId::from_u16(0xF0EF).is_none());

        // Test upper bound + 1
        assert!(DiagComplianceId::from_u16(0xF100).is_none());

        // Test random values
        assert!(DiagComplianceId::from_u16(0x0000).is_none());
        assert!(DiagComplianceId::from_u16(0x1234).is_none());
    }

    #[test]
    fn from_str_returns_err_for_unsupported_strings() {
        assert!(DiagComplianceId::from_str("Reserved_0").is_err()); // Case sensitivity check
        assert!(DiagComplianceId::from_str("unknown").is_err());
    }
}
