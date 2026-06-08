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
