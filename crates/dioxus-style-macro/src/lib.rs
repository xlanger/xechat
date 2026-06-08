//! Procedural macro entry point for dioxus_style

use proc_macro::TokenStream;

mod macros;
mod scss_compiler;
mod style_parser;
mod class_extractor;
mod codegen_utils;
mod hash;
mod css_constants;

/// Attribute macro for attaching scoped or global CSS to a component.
///
/// # Syntax
/// - Scoped: `#[with_css(namespace, "path/to/style.scss")]`
/// - Global: `#[with_css(namespace, "path/to/style.scss", global)]`
#[proc_macro_attribute]
pub fn with_css(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::with_css_impl(attr, item)
}

/// Procedural macro for defining a component with inline CSS declarations.
///
/// # Syntax
/// - `component_with_css! { namespace: "path", fn Component() -> Element { ... } }`
/// - `component_with_css! { namespace: "path", global, fn Component() -> Element { ... } }`
#[proc_macro]
pub fn component_with_css(input: TokenStream) -> TokenStream {
    macros::component_with_css_impl(input)
}

/// Procedural macro for creating a scoped style instance.
///
/// # Syntax
/// - `scoped_style!("path/to/style.scss")`
/// - `scoped_style!(namespace, "path/to/style.scss")`
/// - `scoped_style!(namespace, "path/to/style.scss", global)`
#[proc_macro]
pub fn scoped_style(input: TokenStream) -> TokenStream {
    macros::scoped_style_impl(input)
}
