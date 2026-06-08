//! dioxus_style/src/lib.rs
//! Enhanced scoped CSS styling for Dioxus

mod runtime_injector;
mod css_class;

// Re-export core macros
pub use dioxus_style_macro::{
    component_with_css,
    scoped_style,
    with_css,
};

// Export runtime components
pub use runtime_injector::{inject_scoped_style, inject_styles, ScopedStyle, StyleRegistry, STYLE_REGISTRY};

// Export CSS class helper
pub use css_class::CssClass;
