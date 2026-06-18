//! Scope hash generation for CSS scoping.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Generates a short alphanumeric hash from CSS content and optional file path.
///
/// The hash is designed to be:
/// - Deterministic (same input → same output)
/// - Short (8 chars) for compact CSS class names
/// - URL-safe (alphanumeric only)
/// - Always starts with an alphabetic character (valid CSS identifier)
pub fn generate_hash(css: &str, file_path: Option<&str>) -> String {
    let mut hasher = DefaultHasher::new();
    css.hash(&mut hasher);
    if let Some(path) = file_path {
        path.hash(&mut hasher);
    }
    let hash = hasher.finish();

    // Encode to base52-like alphabetic string (8 chars)
    // Only use a-zA-Z to ensure valid CSS identifiers
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut result = String::with_capacity(8);
    let mut n = hash;
    for _ in 0..8 {
        result.push(ALPHABET[(n % 52) as usize] as char);
        n /= 52;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_hash_deterministic() {
        let h1 = generate_hash("color: red;", Some("file.css"));
        let h2 = generate_hash("color: red;", Some("file.css"));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_generate_hash_different_inputs_different_outputs() {
        let h1 = generate_hash("color: red;", Some("file.css"));
        let h2 = generate_hash("color: blue;", Some("file.css"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_generate_hash_output_length() {
        let h = generate_hash("test", Some("path"));
        assert_eq!(h.len(), 8);
    }

    #[test]
    fn test_generate_hash_alphabetic_only() {
        let h = generate_hash("test content", Some("path"));
        assert!(
            h.chars().all(|c| c.is_ascii_alphabetic()),
            "hash contains non-alphabetic characters: {}",
            h
        );
    }

    #[test]
    fn test_generate_hash_none_vs_some_path() {
        let h1 = generate_hash("color: red;", None);
        let h2 = generate_hash("color: red;", Some("file.css"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_generate_hash_empty_string_no_panic() {
        let h = generate_hash("", None);
        assert_eq!(h.len(), 8);
    }
}
