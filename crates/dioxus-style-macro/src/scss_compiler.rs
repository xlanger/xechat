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

#[must_use = "compiled CSS should be used"]
pub fn compile_scss_to_css(
    content: &str,
    file_path: Option<&str>,
    minify: bool,
) -> Result<String, String> {
    use grass::{Options, OutputStyle};

    let cache_key = hash_content(content, minify);
    if let Ok(cache) = get_cache().read() {
        if let Some(cached) = cache.get(&cache_key) {
            return Ok(cached.clone());
        }
    }

    let mut options = Options::default().style(if minify {
        OutputStyle::Compressed
    } else {
        OutputStyle::Expanded
    });

    // Configure load_path for @import support
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_default();
    let load_paths = vec![
        manifest_dir.join("src/styles"),
        manifest_dir.join("src"),
        manifest_dir.clone(),
    ];
    for path in load_paths {
        if path.exists() {
            options = options.load_path(&path);
        }
    }

    let result = grass::from_string(content.to_string(), &options).map_err(|e| {
        if let Some(path) = file_path {
            format!("SCSS compilation error in '{}': {}", path, e)
        } else {
            format!("SCSS compilation error: {}", e)
        }
    })?;

    if let Ok(mut cache) = get_cache().write() {
        cache.insert(cache_key, result.clone());
    }

    Ok(result)
}

#[inline]
pub fn is_scss_file(path: &str) -> bool {
    path.ends_with(".scss") || path.ends_with(".sass")
}
