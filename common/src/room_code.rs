//! Room code generation and validation.
//!
//! Room codes are human-friendly identifiers for peer-to-peer sessions,
//! formatted as `adjective-noun-number` (e.g., `swift-river-42`).

use rand::prelude::IndexedRandom;

/// Adjectives chosen for distinctiveness and ease of pronunciation.
const ADJECTIVES: &[&str] = &[
    "red", "blue", "green", "gold", "swift", "calm", "bold", "warm", "cool",
    "dark", "bright", "quiet", "loud", "soft", "wild", "free", "deep", "high",
    "long", "quick", "slow", "sharp", "smooth", "fresh", "clear", "pale",
    "rich", "pure", "raw", "dry", "wet", "hot", "cold", "old", "new", "big",
    "tiny", "vast", "slim", "wide", "flat", "round", "fair", "keen", "rare",
    "safe", "sly", "shy", "odd", "apt", "fit", "glad", "grim", "mild", "neat",
];

/// Nouns chosen for distinctiveness and ease of pronunciation.
const NOUNS: &[&str] = &[
    "tiger", "eagle", "river", "stone", "flame", "cloud", "frost", "spark",
    "wave", "leaf", "peak", "moon", "star", "wind", "rain", "snow", "lake",
    "tree", "bird", "fish", "wolf", "bear", "deer", "hawk", "crow", "dove",
    "oak", "pine", "fern", "moss", "sand", "clay", "iron", "jade", "ruby",
    "dawn", "dusk", "noon", "night", "spring", "storm", "creek", "ridge",
    "vale", "marsh", "field", "grove", "shore", "cliff", "cave", "hill",
];

/// Generates a random room code in the format `adjective-noun-XX`.
///
/// The numeric suffix ranges from 00-99, providing 256,000 possible
/// combinations (56 adjectives × 48 nouns × 100 numbers).
///
/// # Example
///
/// ```
/// use common::room_code::generate;
///
/// let code = generate();
/// assert!(code.contains('-'));
/// ```
pub fn generate() -> String {
    let mut rng = rand::rng();
    let adj = ADJECTIVES.choose(&mut rng).expect("adjectives not empty");
    let noun = NOUNS.choose(&mut rng).expect("nouns not empty");
    let num: u8 = rand::random::<u8>() % 100;
    format!("{}-{}-{:02}", adj, noun, num)
}

/// Validates a room code format.
///
/// Returns `true` if the code matches `adjective-noun-XX` format
/// with recognized words and a valid two-digit number.
///
/// Validation is case-insensitive.
///
/// # Example
///
/// ```
/// use common::room_code::validate;
///
/// assert!(validate("swift-river-42"));
/// assert!(validate("SWIFT-RIVER-42"));
/// assert!(!validate("invalid"));
/// assert!(!validate("unknown-word-42"));
/// ```
pub fn validate(code: &str) -> bool {
    let code = code.to_lowercase();
    let parts: Vec<&str> = code.split('-').collect();

    if parts.len() != 3 {
        return false;
    }

    let adj_valid = ADJECTIVES.contains(&parts[0]);
    let noun_valid = NOUNS.contains(&parts[1]);
    let num_valid = parts[2].len() == 2
        && parts[2].chars().all(|c| c.is_ascii_digit())
        && parts[2].parse::<u8>().is_ok();

    adj_valid && noun_valid && num_valid
}

/// Parses a room code into its components.
///
/// Returns `None` if the code is invalid.
///
/// # Example
///
/// ```
/// use common::room_code::parse;
///
/// let (adj, noun, num) = parse("swift-river-42").unwrap();
/// assert_eq!(adj, "swift");
/// assert_eq!(noun, "river");
/// assert_eq!(num, 42);
/// ```
pub fn parse(code: &str) -> Option<(String, String, u8)> {
    if !validate(code) {
        return None;
    }

    let code = code.to_lowercase();
    let parts: Vec<&str> = code.split('-').collect();

    Some((
        parts[0].to_string(),
        parts[1].to_string(),
        parts[2].parse().ok()?,
    ))
}

/// Normalizes a room code to lowercase with consistent formatting.
///
/// Returns `None` if the code is invalid.
///
/// # Example
///
/// ```
/// use common::room_code::normalize;
///
/// assert_eq!(normalize("SWIFT-RIVER-42"), Some("swift-river-42".to_string()));
/// assert_eq!(normalize("Swift-River-5"), None); // invalid: single digit
/// ```
pub fn normalize(code: &str) -> Option<String> {
    parse(code).map(|(adj, noun, num)| format!("{}-{}-{:02}", adj, noun, num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_valid_codes() {
        for _ in 0..100 {
            let code = generate();
            assert!(validate(&code), "generated code should be valid: {}", code);
        }
    }

    #[test]
    fn validate_accepts_valid_codes() {
        assert!(validate("swift-river-42"));
        assert!(validate("calm-moon-00"));
        assert!(validate("bold-tiger-99"));
    }

    #[test]
    fn validate_is_case_insensitive() {
        assert!(validate("SWIFT-RIVER-42"));
        assert!(validate("Swift-River-42"));
        assert!(validate("sWiFt-rIvEr-42"));
    }

    #[test]
    fn validate_rejects_invalid_codes() {
        assert!(!validate(""));
        assert!(!validate("invalid"));
        assert!(!validate("too-many-parts-here"));
        assert!(!validate("unknown-river-42"));
        assert!(!validate("swift-unknown-42"));
        assert!(!validate("swift-river-100")); // number too large
        assert!(!validate("swift-river-9"));   // single digit
        assert!(!validate("swift-river-ab"));  // not a number
    }

    #[test]
    fn parse_extracts_components() {
        let (adj, noun, num) = parse("swift-river-42").unwrap();
        assert_eq!(adj, "swift");
        assert_eq!(noun, "river");
        assert_eq!(num, 42);
    }

    #[test]
    fn parse_returns_none_for_invalid() {
        assert!(parse("invalid").is_none());
    }

    #[test]
    fn normalize_formats_consistently() {
        assert_eq!(normalize("SWIFT-RIVER-05"), Some("swift-river-05".to_string()));
        assert_eq!(normalize("invalid"), None);
    }
}
