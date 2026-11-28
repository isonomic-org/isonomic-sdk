// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The recovery kit's words: the standard 2048-word English list, pinned
//! by its published SHA-256 ([`LIST_SHA256`]), and the checks a window runs
//! while the person types them in. Builds without Isonomic UI.
//!
//! The window checks entry here before it calls `identity.kit_restore`; the
//! engine uses the same list and still does the real verification.
//!
//! Nothing here prints a word: [`Problem`] names a position, never the
//! word at it.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// The list itself, one word per line, in order.
const LIST: &str = include_str!("english.txt");

/// The published SHA-256 of the list file. A test holds the bundled file
/// to it.
pub const LIST_SHA256: &str = "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda";

/// How many words a recovery kit has.
pub const KIT_WORDS: usize = 24;

/// The 2048 words, in list order (which is alphabetical).
pub fn words() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| LIST.lines().collect())
}

/// A word's place in the list.
pub fn index(word: &str) -> Option<usize> {
    words().binary_search(&word).ok()
}

/// Whether `word` is on the list, exactly (lowercase, no spaces).
pub fn is_word(word: &str) -> bool {
    index(word).is_some()
}

/// Every word that starts with `prefix`, in list order: for suggestions as
/// the person types. Empty when none does.
pub fn completions(prefix: &str) -> &'static [&'static str] {
    let list = words();
    let start = list.partition_point(|w| *w < prefix);
    let end = start + list[start..].partition_point(|w| w.starts_with(prefix));
    &list[start..end]
}

/// The one word `prefix` stands for, if only one starts with it. On this
/// list the first four letters always settle it.
pub fn complete(prefix: &str) -> Option<&'static str> {
    match completions(prefix) {
        [one] => Some(one),
        many => many.iter().copied().find(|w| *w == prefix),
    }
}

/// What is wrong with a set of words. Names a position (from 0), never a
/// word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// Not [`KIT_WORDS`] words.
    Count(usize),
    /// The word at this position is not on the list.
    NotAWord { position: usize },
    /// Every word is on the list, but they don't add up: one is wrong or
    /// out of place.
    Checksum,
}

/// Check 24 typed words: each is on the list, and the last carries the
/// checksum of the rest (256 bits, then the first 8 bits of their
/// SHA-256). Trim and lowercase the words first.
pub fn check<W: AsRef<str>>(words: &[W]) -> Result<(), Problem> {
    if words.len() != KIT_WORDS {
        return Err(Problem::Count(words.len()));
    }
    let mut bits = Vec::with_capacity(KIT_WORDS * 11);
    for (position, word) in words.iter().enumerate() {
        let index = index(word.as_ref()).ok_or(Problem::NotAWord { position })?;
        bits.extend((0..11).rev().map(|bit| (index >> bit) & 1 == 1));
    }
    let byte = |chunk: &[bool]| chunk.iter().fold(0u8, |b, &bit| (b << 1) | bit as u8);
    let entropy: Vec<u8> = bits[..256].chunks(8).map(byte).collect();
    let checksum = byte(&bits[256..]);
    if Sha256::digest(&entropy)[0] == checksum {
        Ok(())
    } else {
        Err(Problem::Checksum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard's public 256-bit test vectors (entropy 00…, 7f…, 80…,
    /// ff…). No other words appear in tests.
    const VECTORS: [&str; 4] = [
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
        "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title",
        "letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless",
        "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
    ];

    fn split(vector: &str) -> Vec<&str> {
        vector.split(' ').collect()
    }

    #[test]
    fn the_bundled_list_is_the_published_one() {
        let digest = Sha256::digest(LIST.as_bytes());
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, LIST_SHA256);
        let list = words();
        assert_eq!(list.len(), 2048);
        assert!(list.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
        let mut fours: Vec<&str> = list.iter().map(|w| &w[..w.len().min(4)]).collect();
        fours.dedup();
        assert_eq!(fours.len(), 2048, "four letters settle every word");
    }

    #[test]
    fn the_public_vectors_check() {
        for vector in VECTORS {
            assert_eq!(check(&split(vector)), Ok(()));
        }
    }

    #[test]
    fn a_wrong_word_count_or_order_is_caught_by_position() {
        let mut words = split(VECTORS[0]);
        words[23] = "abandon";
        assert_eq!(check(&words), Err(Problem::Checksum));
        words[5] = "abandonn";
        assert_eq!(check(&words), Err(Problem::NotAWord { position: 5 }));
        assert_eq!(check(&words[..12]), Err(Problem::Count(12)));
        assert!(!format!("{:?}", check(&words)).contains("aband"));
    }

    #[test]
    fn entry_helpers() {
        assert!(is_word("zoo") && !is_word("Zoo") && !is_word(" zoo"));
        assert_eq!(index("abandon"), Some(0));
        assert_eq!(index("zoo"), Some(2047));
        assert_eq!(completions("aban"), ["abandon"]);
        assert_eq!(completions("zo"), ["zone", "zoo"]);
        assert!(completions("qq").is_empty());
        assert_eq!(complete("zon"), Some("zone"));
        assert_eq!(complete("zo"), None, "two words start with it");
        assert_eq!(
            complete("act"),
            Some("act"),
            "a whole word that starts others"
        );
    }
}
