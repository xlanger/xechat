//! SCSS compilation utilities with load_path support.

use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};
use std::path::PathBuf;
use std::env;

const MAX_CACHE_SIZE: usize = 128;

static SCSS_CACHE: OnceLock<RwLock<ScssCache>> = OnceLock::new();

struct ScssCache {
    entries: HashMap<u64, String>,
    insertion_order: VecDeque<u64>,
}

impl ScssCache {
    fn new() -> Self {
        Self {
            entries: HashMap::with_capacity(32),
            insertion_order: VecDeque::with_capacity(32),
        }
    }

    fn get(&self, key: &u64) -> Option<&String> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: u64, value: String) {
        while self.entries.len() >= MAX_CACHE_SIZE && !self.insertion_order.is_empty() {
            if let Some(oldest_key) = self.insertion_order.front().copied() {
                self.entries.remove(&oldest_key);
                self.insertion_order.pop_front();
            }
        }
        if self.entries.insert(key, value).is_none() {
            self.insertion_order.push_back(key);
        }
    }
}

fn get_cache() -> &'static RwLock<ScssCache> {
    SCSS_CACHE.get_or_init(|| RwLock::new(ScssCache::new()))
}

#[inline]
fn hash_content(content: &str, minify: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    minify.hash(&mut hasher);
    hasher.finish()
}

/// Checks the SCSS cache for a previously compiled result.
///
/// Returns `Some(css)` if the cache contains an entry for the given
/// `content` + `minify` combination, otherwise `None`.
#[inline]
fn try_cache_get(content: &str, minify: bool) -> Option<String> {
    let cache_key = hash_content(content, minify);
    let cache = get_cache().read().ok()?;
    cache.get(&cache_key).cloned()
}

/// Builds grass [`Options`] with the appropriate output style and load paths.
///
/// Load paths are derived from `CARGO_MANIFEST_DIR` to support `@import`
/// resolution against `src/styles`, `src`, and the manifest root.
#[inline]
fn build_grass_options(minify: bool) -> grass::Options<'static> {
    use grass::{Options, OutputStyle};

    let mut options = Options::default().style(if minify {
        OutputStyle::Compressed
    } else {
        OutputStyle::Expanded
    });

    // Configure load_path for @import support
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_default();
    let load_paths = [
        manifest_dir.join("src/styles"),
        manifest_dir.join("src"),
        manifest_dir.clone(),
    ];
    for path in load_paths {
        if path.exists() {
            options = options.load_path(&path);
        }
    }
    options
}

/// Inserts a compiled CSS result into the SCSS cache.
///
/// Silently no-ops if the cache lock cannot be acquired.
#[inline]
fn cache_insert(content: &str, minify: bool, result: String) {
    let cache_key = hash_content(content, minify);
    if let Ok(mut cache) = get_cache().write() {
        cache.insert(cache_key, result);
    }
}

/// Compiles SCSS source to CSS with caching and load-path support.
///
/// # Arguments
///
/// * `content` - SCSS source text.
/// * `file_path` - Optional path used to enrich error messages.
/// * `minify` - Whether to emit compressed (minified) output.
///
/// # Errors
///
/// Returns `Err(String)` if the SCSS source fails to compile.
#[must_use = "compiled CSS should be used"]
pub fn compile_scss_to_css(
    content: &str,
    file_path: Option<&str>,
    minify: bool,
) -> Result<String, String> {
    if let Some(cached) = try_cache_get(content, minify) {
        return Ok(cached);
    }

    let options = build_grass_options(minify);
    let result = grass::from_string(content.to_string(), &options).map_err(|e| {
        if let Some(path) = file_path {
            format!("SCSS compilation error in '{}': {}", path, e)
        } else {
            format!("SCSS compilation error: {}", e)
        }
    })?;

    cache_insert(content, minify, result.clone());
    Ok(result)
}

#[inline]
pub fn is_scss_file(path: &str) -> bool {
    path.ends_with(".scss") || path.ends_with(".sass")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_scss_file ----

    #[test]
    fn test_is_scss_file() {
        assert!(is_scss_file("style.scss"));
        assert!(is_scss_file("style.sass"));
        assert!(!is_scss_file("style.css"));
        assert!(!is_scss_file("style.txt"));
        assert!(!is_scss_file(""));
    }

    // ---- compile_scss_to_css ----

    #[test]
    fn test_compile_simple_scss() {
        let result = compile_scss_to_css(".a { color: red; }", None, false);
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
        let css = result.unwrap();
        assert!(css.contains(".a"));
    }

    #[test]
    fn test_compile_scss_with_variables() {
        let scss = "$color: red; .a { color: $color; }";
        let result = compile_scss_to_css(scss, None, false);
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
        let css = result.unwrap();
        assert!(css.contains("red"));
    }

    #[test]
    fn test_compile_minified_output() {
        let result = compile_scss_to_css(".a { color: red; }", None, true);
        assert!(result.is_ok());
        let css = result.unwrap();
        assert!(
            !css.contains('\n'),
            "minified output should not contain newlines: {}",
            css
        );
    }

    #[test]
    fn test_compile_expanded_output() {
        let result = compile_scss_to_css(".a { color: red; }", None, false);
        assert!(result.is_ok());
        let css = result.unwrap();
        assert!(css.contains("red"));
    }

    #[test]
    fn test_compile_cached_returns_same_result() {
        let scss = ".cache_test_selector { color: red; }";
        let r1 = compile_scss_to_css(scss, None, false).unwrap();
        let r2 = compile_scss_to_css(scss, None, false).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_compile_invalid_scss_returns_error() {
        // Unclosed brace should produce a parse error
        let result = compile_scss_to_css(".a { color: red;", None, false);
        assert!(result.is_err(), "expected error for invalid SCSS");
    }

    // ---- ScssCache ----

    #[test]
    fn test_scss_cache_insert_and_get() {
        let mut cache = ScssCache::new();
        cache.insert(1, "value1".to_string());
        assert_eq!(cache.get(&1), Some(&"value1".to_string()));
        assert_eq!(cache.get(&2), None);
    }

    #[test]
    fn test_scss_cache_overwrite_existing_key() {
        let mut cache = ScssCache::new();
        cache.insert(1, "old".to_string());
        cache.insert(1, "new".to_string());
        assert_eq!(cache.get(&1), Some(&"new".to_string()));
    }

    #[test]
    fn test_scss_cache_lru_eviction() {
        let mut cache = ScssCache::new();
        // Fill cache up to MAX_CACHE_SIZE
        for i in 0..MAX_CACHE_SIZE {
            cache.insert(i as u64, format!("value{}", i));
        }
        assert_eq!(cache.get(&0), Some(&"value0".to_string()));
        // Insert one more — should evict the oldest entry (key 0)
        cache.insert(MAX_CACHE_SIZE as u64, "new_value".to_string());
        assert_eq!(
            cache.get(&0),
            None,
            "oldest entry should have been evicted"
        );
        assert_eq!(
            cache.get(&(MAX_CACHE_SIZE as u64)),
            Some(&"new_value".to_string())
        );
        // Key 1 should still be present
        assert_eq!(cache.get(&1), Some(&"value1".to_string()));
    }
}
