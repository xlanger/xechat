//! Procedural macro implementations for scoped styling with SCSS support.

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, LitStr, ItemFn, Ident, Token, parse::{Parse, ParseStream, Parser}};
use std::path::{Path, PathBuf};
use std::env;

use crate::hash::generate_hash;
use crate::style_parser::parse_and_scope;

/// Scans a token stream for `namespace::class_name` references.
///
/// Converts the entire token stream to its string representation and searches
/// for `namespace :: class_name` patterns. This handles both direct token
/// references (`css::sidebar`) and format-string embedded references
/// (`"{css::sidebar}"`) because proc_macro2 displays the token stream with
/// space-separated tokens, making `css::x` appear as `css :: x`.
fn find_used_class_names(tokens: &proc_macro2::TokenStream, namespace: &str) -> Vec<String> {
    use std::collections::HashSet;
    let s = tokens.to_string();
    let mut used = HashSet::new();
    let mut search_start = 0;

    while let Some(pos) = s[search_start..].find(namespace) {
        let after_ns = search_start + pos + namespace.len();
        let rest = &s[after_ns..];
        let rest_trimmed = rest.trim_start();
        if let Some(after_colons) = rest_trimmed.strip_prefix("::") {
            if let Some(class_name) = extract_class_after_namespace(after_colons.trim_start()) {
                used.insert(class_name);
            }
        }
        search_start = after_ns;
    }

    let mut result: Vec<String> = used.into_iter().collect();
    result.sort();
    result
}

/// Given the string after `namespace::`, extracts the class name.
///
/// A class name consists of alphanumeric characters and underscores. Returns
/// `None` if no valid class name can be extracted.
#[inline]
fn extract_class_after_namespace(rest: &str) -> Option<String> {
    if let Some(class_end) = rest.find(|c: char| !c.is_alphanumeric() && c != '_') {
        let class_name = &rest[..class_end];
        if !class_name.is_empty() {
            return Some(class_name.to_string());
        }
    } else if !rest.is_empty() {
        return Some(rest.to_string());
    }
    None
}

/// Filters compiled CSS to only retain rules whose selectors target
/// classes in `used_classes`. Non-class rules (`@media`, `@keyframes`, etc.)
/// are always retained.
fn filter_css_by_classes(css: &str, used_classes: &[String]) -> String {
    let mut css_class_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ident in used_classes {
        css_class_names.insert(ident.replace('_', "-"));
        css_class_names.insert(ident.clone());
    }

    let mut result = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut pos: usize = 0;

    while pos < bytes.len() {
        pos = skip_ascii_whitespace(bytes, pos);
        let (new_pos, rule) = extract_next_rule(bytes, pos);
        pos = new_pos;
        if rule_contains_used_class(rule, &css_class_names) {
            result.push_str(rule);
        }
    }

    result
}

/// Skips ASCII whitespace starting at `pos`, returning the new position.
#[inline]
fn skip_ascii_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

/// Finds the end position of a CSS rule (with balanced braces) starting at `start`.
///
/// Returns the position after the rule's closing brace (or end of input).
#[inline]
fn find_rule_end(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    let mut depth: usize = 0;
    while pos < bytes.len() && !update_depth_and_check_end(bytes[pos], &mut depth) {
        pos += 1;
    }
    if pos < bytes.len() {
        pos += 1;
    }
    pos
}

/// Updates brace depth and returns `true` if the rule has ended (depth back to 0).
#[inline]
fn update_depth_and_check_end(b: u8, depth: &mut usize) -> bool {
    match b {
        b'{' => { *depth += 1; false }
        b'}' => {
            *depth = depth.saturating_sub(1);
            *depth == 0
        }
        _ => false
    }
}

/// Extracts one CSS rule (with balanced braces) starting at `start`.
///
/// Returns `(new_pos, rule_str)` where `new_pos` is the position after the
/// rule's closing brace (or end of input), and `rule_str` is the rule text.
#[inline]
fn extract_next_rule(bytes: &[u8], start: usize) -> (usize, &str) {
    let end = find_rule_end(bytes, start);
    let rule = std::str::from_utf8(&bytes[start..end]).unwrap_or("");
    (end, rule)
}

/// Returns `true` if the first class selector in the rule targets a class
/// in `class_names`. Also returns `true` for rules that do not use class
/// selectors (e.g. `@media`, element selectors).
fn rule_contains_used_class(rule: &str, class_names: &std::collections::HashSet<String>) -> bool {
    if !rule.starts_with('.') {
        return true;
    }
    let selector_part = rule.split('{').next().unwrap_or(rule);
    for part in selector_part.split(',') {
        if check_single_selector_for_class(part, class_names) {
            return true;
        }
    }
    false
}

/// Checks a single comma-separated selector for any class in `class_names`.
#[inline]
fn check_single_selector_for_class(
    selector_part: &str,
    class_names: &std::collections::HashSet<String>,
) -> bool {
    for class_start in selector_part.match_indices('.') {
        let after_dot = &selector_part[class_start.0 + 1..];
        let class_end = after_dot
            .find(|c: char| matches!(c, '{' | ':' | ' ' | ',' | '.' | '[' | '>' | '+' | '~'))
            .unwrap_or(after_dot.len());
        let selector_class = &after_dot[..class_end];
        if class_names.contains(selector_class) {
            return true;
        }
    }
    false
}

use crate::scss_compiler::{compile_scss_to_css, is_scss_file};
use crate::class_extractor::extract_class_names;
use crate::css_constants::looks_like_scss;
use crate::codegen_utils::{generate_class_constant, generate_namespace_module, generate_include_quote};

mod kw {
    syn::custom_keyword!(global);
}

pub struct ProcessedStyle {
    pub scope_hash: String,
    pub scoped_css: String,
    pub class_constants: Vec<proc_macro2::TokenStream>,
    pub include_path: Option<String>,
}

pub fn resolve_and_process_style(
    input_str: &str,
    span: proc_macro2::Span,
    is_global: bool,
    used_classes: &[String],
) -> syn::Result<ProcessedStyle> {
    let (css_content, absolute_path_opt) = load_css_content(input_str, span)?;
    let compiled_css = compile_if_scss(&css_content, input_str, absolute_path_opt.as_deref(), span)?;
    let path_arg_for_hash = absolute_path_opt.as_deref().map(|_| input_str);
    let scope_hash = generate_hash(&compiled_css, path_arg_for_hash);
    let minify = cfg!(not(debug_assertions));
    let scoped_css = scope_css(&compiled_css, &scope_hash, is_global, used_classes, minify);
    let class_constants = generate_class_constants(&compiled_css, used_classes, &scope_hash, is_global);
    let include_path = build_include_path(absolute_path_opt, span)?;
    Ok(ProcessedStyle {
        scope_hash,
        scoped_css,
        class_constants,
        include_path,
    })
}

/// Loads CSS content from a file path or uses the input string directly.
#[inline]
fn load_css_content(input_str: &str, span: proc_macro2::Span) -> syn::Result<(String, Option<PathBuf>)> {
    if !is_likely_file_path(input_str) {
        return Ok((input_str.to_string(), None));
    }
    find_style_file(input_str)
        .map(|(path, content)| (content, Some(path)))
        .map_err(|e| syn::Error::new(span, e))
}

/// Compiles SCSS content to CSS if needed, otherwise returns the content as-is.
#[inline]
fn compile_if_scss(
    css_content: &str,
    input_str: &str,
    absolute_path_opt: Option<&Path>,
    span: proc_macro2::Span,
) -> syn::Result<String> {
    let is_scss = absolute_path_opt
        .map(|_| is_scss_file(input_str))
        .unwrap_or_else(|| looks_like_scss(css_content));
    if !is_scss {
        return Ok(css_content.to_string());
    }
    let minify = cfg!(not(debug_assertions));
    let path_arg = absolute_path_opt.map(|_| input_str);
    compile_scss_to_css(css_content, path_arg, minify)
        .map_err(|e| syn::Error::new(span, e))
}

/// Filters and scopes CSS based on used classes and global flag.
#[inline]
fn scope_css(
    compiled_css: &str,
    scope_hash: &str,
    is_global: bool,
    used_classes: &[String],
    minify: bool,
) -> String {
    let css_to_scope = if used_classes.is_empty() {
        compiled_css.to_string()
    } else {
        filter_css_by_classes(compiled_css, used_classes)
    };
    if is_global {
        css_to_scope
    } else {
        parse_and_scope(&css_to_scope, scope_hash, minify)
    }
}

/// Generates class constant token streams from compiled CSS.
#[inline]
fn generate_class_constants(
    compiled_css: &str,
    used_classes: &[String],
    scope_hash: &str,
    is_global: bool,
) -> Vec<proc_macro2::TokenStream> {
    let selector_infos = extract_class_names(compiled_css);
    let filtered_infos: Vec<_> = if used_classes.is_empty() {
        selector_infos
    } else {
        selector_infos
            .into_iter()
            .filter(|info| used_classes.iter().any(|c| c == info.name()))
            .collect()
    };
    filtered_infos
        .iter()
        .map(|info| generate_class_constant(info, scope_hash, is_global))
        .collect()
}

/// Builds the include path string from an optional absolute path.
#[inline]
fn build_include_path(absolute_path_opt: Option<PathBuf>, span: proc_macro2::Span) -> syn::Result<Option<String>> {
    absolute_path_opt
        .map(|path| path_to_include_str_format(&path))
        .transpose()
        .map_err(|e| syn::Error::new(span, e))
}

struct FunctionMetadata<'a> {
    name: &'a Ident,
    inputs: &'a syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    output: &'a syn::ReturnType,
    vis: &'a syn::Visibility,
    body: &'a syn::Block,
    component_args: Option<proc_macro2::TokenStream>,
    other_attrs: Vec<&'a syn::Attribute>,
}

impl<'a> FunctionMetadata<'a> {
    fn from_item_fn(func: &'a ItemFn) -> Self {
        let mut component_args = None;
        let mut other_attrs = Vec::new();
        for attr in &func.attrs {
            if attr.path().is_ident("component") {
                component_args = Some(if let syn::Meta::List(meta) = &attr.meta {
                    meta.tokens.clone()
                } else {
                    proc_macro2::TokenStream::new()
                });
            } else {
                other_attrs.push(attr);
            }
        }
        Self {
            name: &func.sig.ident,
            inputs: &func.sig.inputs,
            output: &func.sig.output,
            vis: &func.vis,
            body: &func.block,
            component_args,
            other_attrs,
        }
    }

    fn validate_element_return(&self) -> Result<(), proc_macro2::TokenStream> {
        let has_element_return = match self.output {
            syn::ReturnType::Type(_, ty) => {
                if let syn::Type::Path(type_path) = ty.as_ref() {
                    type_path.path.segments.last()
                        .map(|seg| seg.ident == "Element")
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        };
        if !has_element_return {
            return Err(syn::Error::new_spanned(
                self.output,
                "Functions with CSS must return Element"
            ).to_compile_error());
        }
        Ok(())
    }

    fn component_attribute(&self) -> proc_macro2::TokenStream {
        if let Some(ref args) = self.component_args {
            if args.is_empty() {
                quote! { #[::dioxus::prelude::component] }
            } else {
                quote! { #[::dioxus::prelude::component(#args)] }
            }
        } else {
            quote! { #[::dioxus::prelude::component] }
        }
    }
}

pub struct StyleEntry {
    pub namespace: Ident,
    pub _comma: Token![,],
    pub css_file: LitStr,
    pub global: Option<(Token![,], kw::global)>,
}

/// Peeks for `, global` without consuming, returns `true` if present.
#[inline]
fn try_parse_global(input: ParseStream) -> bool {
    input.peek(Token![,]) && input.peek2(kw::global)
}

/// Parses an optional `, global` suffix, consuming it if present.
#[inline]
fn parse_optional_global(input: ParseStream) -> Option<(Token![,], kw::global)> {
    if try_parse_global(input) {
        let comma: Token![,] = input.parse().ok()?;
        let g: kw::global = input.parse().ok()?;
        Some((comma, g))
    } else {
        None
    }
}

impl Parse for StyleEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let namespace: Ident = input.parse()?;
        let _comma: Token![,] = input.parse()?;
        let css_file: LitStr = input.parse()?;
        let global = parse_optional_global(input);
        Ok(StyleEntry { namespace, _comma, css_file, global })
    }
}

pub struct WithCssArgs {
    pub entries: Vec<StyleEntry>,
}

/// Returns `true` when the loop should stop (next token is `, global` or no comma).
#[inline]
fn should_stop_parsing_entries(input: ParseStream) -> bool {
    try_parse_global(input) || !input.peek(Token![,])
}

/// Parses one `StyleEntry` and an optional separator comma.
///
/// Returns `Ok(true)` if the loop should continue, `Ok(false)` if it should stop.
#[inline]
fn parse_next_entry(input: ParseStream, entries: &mut Vec<StyleEntry>) -> syn::Result<bool> {
    entries.push(input.parse()?);
    if should_stop_parsing_entries(input) {
        return Ok(false);
    }
    let _: Token![,] = input.parse()?;
    Ok(true)
}

impl Parse for WithCssArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while parse_next_entry(input, &mut entries)? {}
        Ok(WithCssArgs { entries })
    }
}

/// Computes the used class names for a namespace, or an empty vector if global.
#[inline]
fn compute_used_classes(func: &ItemFn, namespace_str: &str, is_global: bool) -> Vec<String> {
    if is_global {
        Vec::new()
    } else {
        let fn_body_tokens = func.block.to_token_stream();
        find_used_class_names(&fn_body_tokens, namespace_str)
    }
}

/// Builds the scope injection token stream for a namespace, or `None` if global.
#[inline]
fn make_scope_injection(namespace_name: &Ident, is_global: bool) -> Option<proc_macro2::TokenStream> {
    if is_global {
        None
    } else {
        Some(quote! {
            {
                let _css = #namespace_name::get_style_content();
                rsx! { style { "style": "display:none", dangerous_inner_html: "{_css}" } }
            }
        })
    }
}

/// Processes all style entries, returning `(namespace_modules, namespace_inits, scope_injections)`.
///
/// Shared by `with_css_impl` and `component_with_css_impl` to avoid duplication.
fn process_style_entries(
    entries: Vec<(Ident, LitStr, bool)>,
    func: &ItemFn,
) -> syn::Result<(
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
)> {
    let mut namespace_modules = Vec::new();
    let mut namespace_inits = Vec::new();
    let mut scope_injections: Vec<proc_macro2::TokenStream> = Vec::new();

    for (namespace_name, css_file_lit, is_global) in entries {
        let namespace_str = namespace_name.to_string();
        let used_classes = compute_used_classes(func, &namespace_str, is_global);

        let processed = resolve_and_process_style(
            &css_file_lit.value(),
            css_file_lit.span(),
            is_global,
            &used_classes,
        )?;

        let include_quote = generate_include_quote(processed.include_path);
        let namespace_module = generate_namespace_module(
            &namespace_name,
            &processed.scope_hash,
            &processed.scoped_css,
            &processed.class_constants,
            &include_quote,
        );
        namespace_modules.push(namespace_module);
        namespace_inits.push(quote! {
            let _ = #namespace_name::get_style_content();
        });
        if let Some(injection) = make_scope_injection(&namespace_name, is_global) {
            scope_injections.push(injection);
        }
    }

    Ok((namespace_modules, namespace_inits, scope_injections))
}

/// Builds the final expanded token stream from processed style entries and function metadata.
#[inline]
fn build_expanded_function(
    meta: &FunctionMetadata,
    namespace_modules: &[proc_macro2::TokenStream],
    namespace_inits: &[proc_macro2::TokenStream],
    scope_injections: &[proc_macro2::TokenStream],
) -> TokenStream {
    let component_attr = meta.component_attribute();
    let other_attrs = &meta.other_attrs;
    let fn_name = meta.name;
    let fn_inputs = meta.inputs;
    let fn_output = meta.output;
    let fn_vis = meta.vis;
    let fn_body = meta.body;

    let expanded = quote! {
        #component_attr
        #(#other_attrs)*
        #fn_vis fn #fn_name(#fn_inputs) #fn_output {
            use ::dioxus::prelude::*;

            #(#namespace_modules)*

            #(#namespace_inits)*

            let user_element = { #fn_body };

            rsx! {
                #(#scope_injections)*
                {user_element}
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn with_css_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as WithCssArgs);

    let func = parse_macro_input!(item as ItemFn);
    let meta = FunctionMetadata::from_item_fn(&func);

    if let Err(e) = meta.validate_element_return() {
        return e.into();
    }

    let entries: Vec<(Ident, LitStr, bool)> = args.entries
        .into_iter()
        .map(|e| (e.namespace, e.css_file, e.global.is_some()))
        .collect();

    let (namespace_modules, namespace_inits, scope_injections) = match process_style_entries(entries, &func) {
        Ok(result) => result,
        Err(e) => return e.to_compile_error().into(),
    };

    build_expanded_function(&meta, &namespace_modules, &namespace_inits, &scope_injections)
}

pub struct ComponentInput {
    pub style_files: Vec<(Ident, LitStr, bool)>,
    pub func: ItemFn,
}

impl Parse for ComponentInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let style_files = parse_all_style_entries(input)?;
        let func: ItemFn = input.parse()?;
        validate_style_files(&style_files, input.span())?;
        Ok(ComponentInput { style_files, func })
    }
}

/// Parses all `namespace:path[, global]` entries until `fn` keyword.
#[inline]
fn parse_all_style_entries(input: ParseStream) -> syn::Result<Vec<(Ident, LitStr, bool)>> {
    let mut style_files = Vec::new();
    while try_parse_next_entry(input, &mut style_files)? {}
    Ok(style_files)
}

/// Attempts to parse one entry; returns `Ok(false)` when `fn` is reached.
#[inline]
fn try_parse_next_entry(
    input: ParseStream,
    style_files: &mut Vec<(Ident, LitStr, bool)>,
) -> syn::Result<bool> {
    if input.peek(Token![fn]) {
        return Ok(false);
    }
    style_files.push(parse_style_file_entry(input)?);
    Ok(consume_optional_separator(input)?)
}

/// Validates that at least one style file was parsed.
#[inline]
fn validate_style_files(
    style_files: &[(Ident, LitStr, bool)],
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if style_files.is_empty() {
        return Err(syn::Error::new(span, "Expected at least one namespace:path pair before function"));
    }
    Ok(())
}

/// Parses one `namespace:path[, global]` entry.
#[inline]
fn parse_style_file_entry(input: ParseStream) -> syn::Result<(Ident, LitStr, bool)> {
    let namespace: Ident = input.parse()?;
    let _: Token![:] = input.parse()?;
    let css_path: LitStr = input.parse()?;
    let is_global = parse_optional_global(input).is_some();
    Ok((namespace, css_path, is_global))
}

/// Consumes an optional comma separator, returns `true` if consumed.
#[inline]
fn consume_optional_separator(input: ParseStream) -> syn::Result<bool> {
    if !input.peek(Token![,]) {
        return Ok(false);
    }
    let _: Token![,] = input.parse()?;
    Ok(true)
}

pub fn component_with_css_impl(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as ComponentInput);
    let style_files = args.style_files;
    let func = args.func;

    let meta = FunctionMetadata::from_item_fn(&func);
    if let Err(e) = meta.validate_element_return() {
        return e.into();
    }

    let (namespace_modules, namespace_inits, scope_injections) = match process_style_entries(style_files, &func) {
        Ok(result) => result,
        Err(e) => return e.to_compile_error().into(),
    };

    build_expanded_function(&meta, &namespace_modules, &namespace_inits, &scope_injections)
}

pub fn scoped_style_impl(input: TokenStream) -> TokenStream {
    scoped_style_impl_inner(input).unwrap_or_else(|e| e.to_compile_error().into())
}

/// Inner implementation that can use `?` for error propagation.
fn scoped_style_impl_inner(input: TokenStream) -> syn::Result<TokenStream> {
    let (namespace, css_lit, is_global) = parse_scoped_style_input.parse(input)?;
    let processed = resolve_and_process_style(&css_lit.value(), css_lit.span(), is_global, &[])?;
    Ok(build_scoped_style_from_namespace(namespace, processed))
}

/// Dispatches to the named or anonymous scoped style builder.
#[inline]
fn build_scoped_style_from_namespace(namespace: Option<Ident>, processed: ProcessedStyle) -> TokenStream {
    match namespace {
        Some(mod_name) => build_scoped_style_output(&mod_name, processed),
        None => build_anonymous_scoped_style(processed),
    }
}

/// Parses scoped_style input into (namespace, css_lit, is_global).
#[inline]
fn parse_scoped_style_input(input: ParseStream) -> syn::Result<(Option<Ident>, LitStr, bool)> {
    if input.peek(Ident) && input.peek2(Token![,]) {
        return parse_named_scoped_style_input(input);
    }
    Ok((None, input.parse()?, false))
}

/// Parses a named scoped_style input: `namespace, "path"[, global]`.
#[inline]
fn parse_named_scoped_style_input(input: ParseStream) -> syn::Result<(Option<Ident>, LitStr, bool)> {
    let name: Ident = input.parse()?;
    let _: Token![,] = input.parse()?;
    let path: LitStr = input.parse()?;
    let is_global = parse_optional_global(input).is_some();
    Ok((Some(name), path, is_global))
}

/// Builds the output for a named namespace scoped style.
#[inline]
fn build_scoped_style_output(mod_name: &Ident, processed: ProcessedStyle) -> TokenStream {
    let namespace_module = generate_namespace_module(
        mod_name,
        &processed.scope_hash,
        &processed.scoped_css,
        &processed.class_constants,
        &generate_include_quote(processed.include_path),
    );
    let expanded = quote! {
        #namespace_module
        let _ = #mod_name::get_scope();
    };
    TokenStream::from(expanded)
}

/// Builds the output for an anonymous (no namespace) scoped style.
#[inline]
fn build_anonymous_scoped_style(processed: ProcessedStyle) -> TokenStream {
    let scope_hash = &processed.scope_hash;
    let scoped_css = &processed.scoped_css;
    let include_quote = generate_include_quote(processed.include_path);
    let expanded = quote! {
        {
            use ::std::sync::OnceLock;
            static STYLE_INSTANCE: OnceLock<::dioxus_style::ScopedStyle> = OnceLock::new();
            *STYLE_INSTANCE.get_or_init(|| {
                #include_quote
                ::dioxus_style::ScopedStyle::new(#scope_hash, #scoped_css)
            })
        }
    };
    TokenStream::from(expanded)
}

pub fn path_to_include_str_format(path: &Path) -> Result<String, String> {
    let canonical = path.canonicalize()
        .map_err(|e| format!("Failed to canonicalize path '{}': {}", path.display(), e))?;
    let path_str = canonical.to_str()
        .ok_or_else(|| format!("Path contains invalid UTF-8: {}", canonical.display()))?;
    let normalized = if cfg!(windows) {
        path_str.replace('\\', "/")
    } else {
        path_str.to_string()
    };
    Ok(normalized)
}

pub fn find_style_file(file_path: &str) -> Result<(PathBuf, String), String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR not set (not running in cargo context)".to_string())?;
    let base = PathBuf::from(&manifest_dir);
    let candidates = build_candidate_paths(&base, file_path);
    try_read_first_file(&candidates).map_err(|tried| {
        format!(
            "Style file '{}' not found.\nSearched in:\n{}\n",
            file_path,
            tried
        )
    })
}

/// Builds the list of candidate paths to search for a style file.
#[inline]
fn build_candidate_paths(base: &Path, file_path: &str) -> Vec<PathBuf> {
    vec![
        base.join(file_path),
        base.join("src").join(file_path),
        base.join("styles").join(file_path),
        base.join("assets").join(file_path),
        base.join("src/styles").join(file_path),
        base.join("src/assets").join(file_path),
    ]
}

/// Returns `true` if the path exists and is a file.
#[inline]
fn is_readable_file(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// Reads file content, mapping IO errors to a descriptive string.
#[inline]
fn read_file_content(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))
}

/// Formats the list of tried paths into a newline-separated string.
#[inline]
fn format_tried_paths(paths: &[PathBuf]) -> String {
    paths.iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Tries each path in order, returning the first existing file's content.
///
/// On success returns `(path, content)`. On failure returns an error containing
/// the formatted list of tried paths.
#[inline]
fn try_read_first_file(paths: &[PathBuf]) -> Result<(PathBuf, String), String> {
    for path in paths {
        if is_readable_file(path) {
            let content = read_file_content(path)?;
            return Ok((path.clone(), content));
        }
    }
    Err(format_tried_paths(paths))
}

fn is_likely_file_path(input: &str) -> bool {
    input.ends_with(".css") || input.ends_with(".scss") || input.ends_with(".sass")
        || input.contains('/') || input.contains('\\')
}
