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

use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

/// Diagnostics Compliance Identifiers for DID and DataItem name mapping.
#[derive(AsRefStr, Debug, PartialEq, Eq, Clone, Copy, Display, EnumString, EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum DiagComplianceId {
    /// Mapping for did_range_start_id +0
    #[strum(serialize = "reserved_0")]
    Reserved0,
    /// Mapping for did_range_start_id +1
    #[strum(serialize = "reserved_1")]
    Reserved1,
    /// Mapping for did_range_start_id +2
    #[strum(serialize = "reserved_2")]
    Reserved2,
    /// Mapping for did_range_start_id +3 -> +15
    #[strum(serialize = "default")]
    Default,
}

impl DiagComplianceId {
    /// Map enum to its u16 byte value, calculated from a dynamic base address.
    pub fn to_u16(&self, did_range_start_id: u16) -> u16 {
        match self {
            Self::Reserved0 => did_range_start_id,
            Self::Reserved1 => did_range_start_id + 1,
            Self::Reserved2 => did_range_start_id + 2,
            Self::Default => did_range_start_id + 15, // End of the 16-element window
        }
    }

    /// Create enum from a u16 value, evaluated against a dynamic base address.
    pub fn from_u16(value: u16, did_range_start_id: u16) -> Option<Self> {
        if value == did_range_start_id {
            Some(Self::Reserved0)
        } else if value == did_range_start_id + 1 {
            Some(Self::Reserved1)
        } else if value == did_range_start_id + 2 {
            Some(Self::Reserved2)
        } else if value >= did_range_start_id + 3 && value <= did_range_start_id + 15 {
            Some(Self::Default)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    static GOOGLE_RESERVED_DID_RANGE_START_TEST: u16 = 0xF0F0;
    // --- Reserved Explicit Mappings ---

    #[test]
    fn reserved0_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F0, GOOGLE_RESERVED_DID_RANGE_START_TEST).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved0);
        assert_eq!(id.to_u16(0xF0F0), 0xF0F0);
        assert_eq!(id.to_string(), "reserved_0");
        assert_eq!(DiagComplianceId::from_str("reserved_0").unwrap(), id);
    }

    #[test]
    fn reserved1_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F1, GOOGLE_RESERVED_DID_RANGE_START_TEST).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved1);
        assert_eq!(id.to_u16(0xF0F0), 0xF0F1);
        assert_eq!(id.to_string(), "reserved_1");
        assert_eq!(DiagComplianceId::from_str("reserved_1").unwrap(), id);
    }

    #[test]
    fn reserved2_variant_maps_correctly() {
        let id = DiagComplianceId::from_u16(0xF0F2, GOOGLE_RESERVED_DID_RANGE_START_TEST).unwrap();
        assert_eq!(id, DiagComplianceId::Reserved2);
        assert_eq!(id.to_u16(0xF0F0), 0xF0F2);
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
        assert_eq!(DiagComplianceId::Default.to_u16(0xF0F0), 0xF0FF);
    }

    #[test]
    fn from_u16_with_default_range_maps_to_default_variant() {
        for did in 0xF0F3..=0xF0FF {
            let id = DiagComplianceId::from_u16(did, GOOGLE_RESERVED_DID_RANGE_START_TEST)
                .unwrap_or_else(|| {
                    panic!("DID {did:#X} should be supported in `DiagComplianceId`")
                });
            assert_eq!(id, DiagComplianceId::Default, "DID {did:#X} should map to Default");
        }
    }

    // --- Negative Tests ---

    #[test]
    fn from_u16_returns_none_for_unsupported_values() {
        // Test lower bound - 1
        assert!(DiagComplianceId::from_u16(0xF0EF, GOOGLE_RESERVED_DID_RANGE_START_TEST).is_none());

        // Test upper bound + 1
        assert!(DiagComplianceId::from_u16(0xF100, GOOGLE_RESERVED_DID_RANGE_START_TEST).is_none());

        // Test random values
        assert!(DiagComplianceId::from_u16(0x0000, GOOGLE_RESERVED_DID_RANGE_START_TEST).is_none());
        assert!(DiagComplianceId::from_u16(0x1234, GOOGLE_RESERVED_DID_RANGE_START_TEST).is_none());
    }

    #[test]
    fn from_str_returns_err_for_unsupported_strings() {
        assert!(DiagComplianceId::from_str("Reserved_0").is_err()); // Case sensitivity check
        assert!(DiagComplianceId::from_str("unknown").is_err());
    }
}
