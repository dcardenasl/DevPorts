//! References and tokens.
//!
//! A *reference* is the short public name of an instance (`a1b2`), shown in
//! `ls` and the log. A *token* is the secret that lets a caller stop or extend
//! its own Temporal; only its SHA-256 is ever stored (ADR-009).

use std::collections::BTreeSet;

use crate::hash::sha256_hex;
use crate::process::ProcessId;

/// How many hexadecimal characters a reference has.
const REFERENCE_LENGTH: usize = 4;

/// The reference of an instance nobody registered, derived from the process
/// identity so it stays the same from one scan to the next.
#[must_use]
pub fn observed_reference(id: ProcessId) -> String {
    let digest = sha256_hex(format!("observed:{}:{}", id.pid, id.start.0).as_bytes());
    digest.chars().take(REFERENCE_LENGTH).collect()
}

/// The reference of an observed container.
#[must_use]
pub fn container_reference(container_id: &str) -> String {
    let digest = sha256_hex(format!("container:{container_id}").as_bytes());
    digest.chars().take(REFERENCE_LENGTH).collect()
}

/// A new reference for a registered instance, different from every one in
/// `taken`. `entropy` is any fresh bytes.
#[must_use]
pub fn new_reference(entropy: &[u8; 16], taken: &BTreeSet<String>) -> String {
    let mut seed = entropy.to_vec();
    loop {
        let digest = sha256_hex(&seed);
        let candidate: String = digest.chars().take(REFERENCE_LENGTH).collect();
        if !taken.contains(&candidate) {
            return candidate;
        }
        // 65 536 references: on a clash, hash again.
        seed = digest.into_bytes();
    }
}

/// The text of a token, from 128 random bits: 32 hexadecimal characters.
#[must_use]
pub fn token_text(entropy: &[u8; 16]) -> String {
    crate::hash::to_hex(entropy)
}

/// What is stored in place of the token.
#[must_use]
pub fn token_hash(token: &str) -> String {
    sha256_hex(token.as_bytes())
}

/// Fresh random bytes from the operating system.
///
/// # Errors
///
/// Only if the system cannot provide randomness, which is a broken machine.
pub fn system_entropy() -> Result<[u8; 16], getrandom::Error> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessStart;

    #[test]
    fn an_observed_reference_is_stable_and_four_hex_characters() {
        let id = ProcessId {
            pid: 4121,
            start: ProcessStart(1_700_000_000_000_000),
        };
        let first = observed_reference(id);
        assert_eq!(first, observed_reference(id));
        assert_eq!(first.len(), 4);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        let other = ProcessId { pid: 4122, ..id };
        assert_ne!(first, observed_reference(other));
    }

    #[test]
    fn a_recycled_pid_gets_a_different_reference() {
        let first = ProcessId {
            pid: 10,
            start: ProcessStart(1),
        };
        let second = ProcessId {
            pid: 10,
            start: ProcessStart(2),
        };
        assert_ne!(observed_reference(first), observed_reference(second));
    }

    #[test]
    fn a_new_reference_avoids_the_ones_in_use() {
        let entropy = [7_u8; 16];
        let first = new_reference(&entropy, &BTreeSet::new());
        let taken: BTreeSet<String> = [first.clone()].into_iter().collect();
        let second = new_reference(&entropy, &taken);
        assert_ne!(first, second);
        assert_eq!(second.len(), 4);
    }

    #[test]
    fn a_token_is_128_bits_in_hex_and_only_its_hash_is_kept() {
        let token = token_text(&[0xab; 16]);
        assert_eq!(token, "ab".repeat(16));
        let hash = token_hash(&token);
        assert_eq!(hash.len(), 64);
        assert!(!hash.contains(&token));
        assert_eq!(hash, token_hash(&token));
    }

    #[test]
    fn system_entropy_is_not_constant() {
        assert_ne!(system_entropy().unwrap(), system_entropy().unwrap());
    }
}
