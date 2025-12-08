// Copyright 2025 Google LLC

/// Represents a 2-byte Logical Address (e.g., 0x0E00, Big Endian).
/// Using a tuple struct provides zero-cost abstraction over [u8; 2].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicalAddress([u8; 2]);

impl LogicalAddress {
    /// Creates a new Logical Address from a u16 (Big Endian).
    pub const fn new(value: u16) -> Self {
        Self(value.to_be_bytes())
    }

    /// Returns the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 2] {
        &self.0
    }

    /// Returns the value as a u16 (Big Endian).
    pub fn as_u16(&self) -> u16 {
        u16::from_be_bytes(self.0)
    }
}

// Allow initializing from a u16 directly
impl From<u16> for LogicalAddress {
    fn from(val: u16) -> Self {
        Self::new(val)
    }
}

// Allow initializing from raw bytes
impl From<[u8; 2]> for LogicalAddress {
    fn from(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }
}
