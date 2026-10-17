//! SHA-256 helpers.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// `bytes` as lowercase hexadecimal.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut out, byte| {
            // Writing to a `String` cannot fail.
            let _ = write!(out, "{byte:02x}");
            out
        },
    )
}

/// The SHA-256 digest of `data`, as 64 lowercase hexadecimal characters.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    to_hex(&Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_published_test_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
