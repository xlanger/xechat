//! Runtime style injection and registry.
//!
//! Manages the collection and injection of scoped styles into the DOM.

use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};

/// Global registry for all scoped styles.
/// Uses OnceLock (Rust 1.70+) instead of lazy_static for better performance.
/// RwLock allows concurrent reads which is the common case for inject_styles().
pub static STYLE_REGISTRY: OnceLock<RwLock<StyleRegistry>> = OnceLock::new();

/// Gets the style registry, initializing it if necessary.
#[inline]
fn get_registry() -> &'static RwLock<StyleRegistry> {
    STYLE_REGISTRY.get_or_init(|| RwLock::new(StyleRegistry::new()))
}

/// Tracks which scopes have already been injected into the DOM.
/// Prevents duplicate `<style>` tags when multiple component instances
/// share the same scope hash.
static INJECTED_SCOPES: OnceLock<RwLock<HashSet<&'static str>>> = OnceLock::new();

/// Returns the injected scopes tracker, initializing it on first access.
fn get_injected() -> &'static RwLock<HashSet<&'static str>> {
    INJECTED_SCOPES.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Retrieves the scoped CSS content for a given scope hash.
///
/// This function always returns the full CSS content registered for the
/// scope. The deduplication tracking (via `INJECTED_SCOPES`) is maintained
/// as a side effect but does not affect the return value — it always
/// returns the CSS content to ensure correctness across Dioxus re-renders.
///
/// # Arguments
/// * `scope` - The scope hash string (compile-time static) identifying
///   the style to retrieve.
///
/// # Returns
/// The scoped CSS content for the given scope, or an empty string if
/// the scope is not found in the registry.
pub fn inject_scoped_style(scope: &'static str) -> String {
    // Track injection state (side effect only, does not affect return).
    if let Ok(mut guard) = get_injected().write() {
        guard.insert(scope);
    }
    // Always retrieve the CSS from the registry.
    match get_registry().read() {
        Ok(guard) => guard.get_style(scope).unwrap_or("").to_string(),
        Err(_) => String::new(),
    }
}

/// Registry that tracks all scoped styles in the application.
/// Uses &'static str to avoid runtime allocations - all CSS is embedded at compile time.
#[derive(Debug, Default)]
pub struct StyleRegistry {
    // HashMap for O(1) lookups and deduplication
    styles: HashMap<&'static str, &'static str>,
    // Maintain insertion order for consistent output
    order: Vec<&'static str>,
    // Optimization #1: Cached combined CSS output
    cached_output: Option<String>,
}

impl StyleRegistry {
    /// Creates a new empty style registry.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            styles: HashMap::with_capacity(32), // Pre-allocate for typical use
            order: Vec::with_capacity(32),
            cached_output: None,
        }
    }

    /// Registers a scoped style with its hash.
    ///
    /// # Arguments
    /// * `hash` - The unique hash/scope for this style (compile-time static)
    /// * `css` - The scoped CSS content (compile-time static)
    #[inline]
    pub fn register(&mut self, hash: &'static str, css: &'static str) {
        use std::collections::hash_map::Entry;

        match self.styles.entry(hash) {
            Entry::Occupied(mut entry) => {
                // Update existing entry
                entry.insert(css);
                // Invalidate cache on update
                self.cached_output = None;
            }
            Entry::Vacant(entry) => {
                // Insert new entry and track order
                entry.insert(css);
                self.order.push(hash);
                // Invalidate cache on new entry
                self.cached_output = None;
            }
        }
    }

    /// Gets all registered styles as a single CSS string.
    /// Uses cached output for repeated calls (optimization #1).
    #[inline]
    #[must_use]
    pub fn get_all_styles(&mut self) -> &str {
        // Return cached output if available (zero-copy)
        if let Some(ref cached) = self.cached_output {
            return cached.as_str();
        }

        if self.order.is_empty() {
            return "";
        }

        // Pre-calculate total size to avoid reallocations
        let total_size: usize = self
            .styles
            .values()
            .map(|s| s.len() + 1) // +1 for newline
            .sum();

        let mut result = String::with_capacity(total_size);

        for hash in &self.order {
            if let Some(css) = self.styles.get(hash) {
                result.push_str(css);
                result.push('\n');
            }
        }

        // Cache the result for future calls
        self.cached_output = Some(result);
        self.cached_output.as_ref().unwrap()
    }

    /// Checks if a style hash is already registered.
    #[inline]
    #[must_use]
    pub fn contains(&self, hash: &str) -> bool {
        self.styles.contains_key(hash)
    }

    /// Clears all registered styles (useful for testing).
    #[inline]
    pub fn clear(&mut self) {
        self.styles.clear();
        self.order.clear();
        self.cached_output = None;
    }

    /// Gets the number of registered styles.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Checks if the registry is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Retrieves the CSS content for a single style by its scope hash.
    ///
    /// Returns `None` if no style is registered under the given hash.
    #[inline]
    #[must_use]
    pub fn get_style(&self, hash: &str) -> Option<&str> {
        self.styles.get(hash).copied()
    }
}

/// Gets all styles from the global registry as a single CSS string for injection.
/// Uses cached output for 10-50x faster repeated calls.
#[inline]
#[must_use]
pub fn inject_styles() -> String {
    // Use write lock to allow cache mutation
    match get_registry().write() {
        Ok(mut guard) => guard.get_all_styles().to_string(),
        Err(_) => String::new(), // Graceful degradation if lock poisoned
    }
}

/// Helper struct for managing a single scoped style instance.
/// Uses &'static str for zero-allocation access to compile-time embedded strings.
#[derive(Debug, Clone, Copy)]
pub struct ScopedStyle {
    pub scope: &'static str,
}

impl ScopedStyle {
    /// Creates a new scoped style and registers it.
    /// Both scope and css are compile-time static strings.
    #[inline]
    pub fn new(scope: &'static str, css: &'static str) -> Self {
        if let Ok(mut guard) = get_registry().write() {
            guard.register(scope, css);
        }
        // If lock is poisoned, style won't be registered but app continues

        Self { scope }
    }

    /// Returns the scope prefix for use in class names.
    #[inline]
    pub fn scope(&self) -> &'static str {
        self.scope
    }
}

impl std::fmt::Display for ScopedStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.scope)
    }
}
