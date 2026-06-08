//! Procedural macro implementations for scoped styling with SCSS support.

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, LitStr, ItemFn, Ident, Token, parse::{Parse, ParseStream}};
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
        if rest_trimmed.starts_with("::") {
            let after_colons = rest_trimmed[2..].trim_start();
            if let Some(class_end) = after_colons.find(|c: char| !c.is_alphanumeric() && c != '_') {
                let class_name = &after_colons[..class_end];
                if !class_name.is_empty() {
                    used.insert(class_name.to_string());
                }
            } else if !after_colons.is_empty() {
                used.insert(after_colons.to_string());
            }
        }
        search_start = after_ns;
    }

    let mut result: Vec<String> = used.into_iter().collect();
    result.sort();
    result
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
    let len = bytes.len();
    let mut pos: usize = 0;

    while pos < len {
        while pos < len && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= len {
            break;
        }

        let rule_start = pos;
        let mut depth: usize = 0;
        while pos < len {
            match bytes[pos] {
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        pos += 1;
                        break;
                    }
                }
                _ => {}
            }
            pos += 1;
        }

        let rule = std::str::from_utf8(&bytes[rule_start..pos]).unwrap_or("");
        if rule_contains_used_class(rule, &css_class_names) {
            result.push_str(rule);
        }
    }

    result
}

/// Returns `true` if the first class selector in the rule targets a class
/// in `class_names`. Also returns `true` for rules that do not use class
/// selectors (e.g. `@media`, element selectors).
fn rule_contains_used_class(rule: &str, class_names: &std::collections::HashSet<String>) -> bool {
    if !rule.starts_with('.') {
        return true;
    }
    let selector_part = rule.split('{').next().unwrap_or(rule);
    let mut found = false;
    for part in selector_part.split(',') {
        for class_start in part.match_indices('.') {
            let after_dot = &part[class_start.0 + 1..];
            let class_end = after_dot
                .find(|c: char| matches!(c, '{' | ':' | ' ' | ',' | '.' | '[' | '>' | '+' | '~'))
                .unwrap_or(after_dot.len());
            let selector_class = &after_dot[..class_end];
            if class_names.contains(selector_class) {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }
    found
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
    let is_likely_path = is_likely_file_path(input_str);
    let (css_content, absolute_path_opt) = if is_likely_path {
        match find_style_file(input_str) {
            Ok((path, content)) => (content, Some(path)),
            Err(e) => return Err(syn::Error::new(span, e)),
        }
    } else {
        (input_str.to_string(), None)
    };

    let is_scss = if absolute_path_opt.is_some() {
        is_scss_file(input_str)
    } else {
        looks_like_scss(&css_content)
    };

    let compiled_css = if is_scss {
        let minify = cfg!(not(debug_assertions));
        let path_arg = if absolute_path_opt.is_some() { Some(input_str) } else { None };
        match compile_scss_to_css(&css_content, path_arg, minify) {
            Ok(css) => css,
            Err(e) => return Err(syn::Error::new(span, e)),
        }
    } else {
        css_content
    };

    let path_arg_for_hash = if absolute_path_opt.is_some() { Some(input_str) } else { None };
    let scope_hash = generate_hash(&compiled_css, path_arg_for_hash);

    let minify = cfg!(not(debug_assertions));

    let css_to_scope = if used_classes.is_empty() {
        compiled_css.clone()
    } else {
        filter_css_by_classes(&compiled_css, used_classes)
    };

    let scoped_css = if is_global {
        css_to_scope.clone()
    } else {
        parse_and_scope(&css_to_scope, &scope_hash, minify)
    };

    let selector_infos = extract_class_names(&compiled_css);
    let filtered_infos: Vec<_> = if used_classes.is_empty() {
        selector_infos
    } else {
        selector_infos
            .into_iter()
            .filter(|info| used_classes.iter().any(|c| c == info.name()))
            .collect()
    };

    let class_constants: Vec<_> = filtered_infos
        .iter()
        .map(|info| generate_class_constant(info, &scope_hash, is_global))
        .collect();

    let include_path = if let Some(path) = absolute_path_opt {
        match path_to_include_str_format(&path) {
            Ok(p) => Some(p),
            Err(e) => return Err(syn::Error::new(span, e)),
        }
    } else {
        None
    };

    Ok(ProcessedStyle {
        scope_hash,
        scoped_css,
        class_constants,
        include_path,
    })
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

impl Parse for StyleEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let namespace: Ident = input.parse()?;
        let _comma: Token![,] = input.parse()?;
        let css_file: LitStr = input.parse()?;
        let global = if input.peek(Token![,]) && input.peek2(kw::global) {
            let comma: Token![,] = input.parse()?;
            let g: kw::global = input.parse()?;
            Some((comma, g))
        } else {
            None
        };
        Ok(StyleEntry { namespace, _comma, css_file, global })
    }
}

pub struct WithCssArgs {
    pub entries: Vec<StyleEntry>,
}

impl Parse for WithCssArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        loop {
            entries.push(input.parse()?);
            if input.peek(Token![,]) && !input.peek2(kw::global) {
                let _: Token![,] = input.parse()?;
            } else {
                break;
            }
        }
        Ok(WithCssArgs { entries })
    }
}

pub fn with_css_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as WithCssArgs);

    let func = parse_macro_input!(item as ItemFn);
    let meta = FunctionMetadata::from_item_fn(&func);

    if let Err(e) = meta.validate_element_return() {
        return e.into();
    }

    let mut namespace_modules = Vec::new();
    let mut namespace_inits = Vec::new();
    let mut scope_injections: Vec<proc_macro2::TokenStream> = Vec::new();

    for entry in args.entries {
        let namespace_name = entry.namespace;
        let namespace_str = namespace_name.to_string();
        let css_file_lit = entry.css_file;
        let is_global = entry.global.is_some();

        let used_classes = if is_global {
            Vec::new()
        } else {
            let fn_body_tokens = func.block.to_token_stream();
            find_used_class_names(&fn_body_tokens, &namespace_str)
        };

        let processed = match resolve_and_process_style(
            &css_file_lit.value(),
            css_file_lit.span(),
            is_global,
            &used_classes,
        ) {
            Ok(p) => p,
            Err(e) => return e.to_compile_error().into()
        };

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
        if !is_global {
            scope_injections.push(quote! {
                {
                    let _css = #namespace_name::get_style_content();
                    rsx! { style { "style": "display:none", dangerous_inner_html: "{_css}" } }
                }
            });
        }
    }

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

pub struct ComponentInput {
    pub style_files: Vec<(Ident, LitStr, bool)>,
    pub func: ItemFn,
}

impl Parse for ComponentInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut style_files = Vec::new();
        while !input.peek(Token![fn]) {
            let namespace: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            let css_path: LitStr = input.parse()?;
            let is_global = if input.peek(Token![,]) && input.peek2(kw::global) {
                let _: Token![,] = input.parse()?;
                let _: kw::global = input.parse()?;
                true
            } else {
                false
            };
            style_files.push((namespace, css_path, is_global));
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            } else {
                break;
            }
        }
        let func: ItemFn = input.parse()?;
        if style_files.is_empty() {
            return Err(syn::Error::new(input.span(), "Expected at least one namespace:path pair before function"));
        }
        Ok(ComponentInput { style_files, func })
    }
}

pub fn component_with_css_impl(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as ComponentInput);
    let style_files = args.style_files;
    let func = args.func;

    let meta = FunctionMetadata::from_item_fn(&func);
    if let Err(e) = meta.validate_element_return() {
        return e.into();
    }

    let mut namespace_modules = Vec::new();
    let mut namespace_inits = Vec::new();
    let mut scope_injections: Vec<proc_macro2::TokenStream> = Vec::new();

    for (namespace_name, css_file_lit, is_global) in style_files {
        let namespace_str = namespace_name.to_string();

        let used_classes = if is_global {
            Vec::new()
        } else {
            let fn_body_tokens = func.block.to_token_stream();
            find_used_class_names(&fn_body_tokens, &namespace_str)
        };

        let processed = match resolve_and_process_style(
            &css_file_lit.value(),
            css_file_lit.span(),
            is_global,
            &used_classes,
        ) {
            Ok(p) => p,
            Err(e) => return e.to_compile_error().into()
        };
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
        if !is_global {
            scope_injections.push(quote! {
                {
                    let _css = #namespace_name::get_style_content();
                    rsx! { style { "style": "display:none", dangerous_inner_html: "{_css}" } }
                }
            });
        }
    }

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

pub fn scoped_style_impl(input: TokenStream) -> TokenStream {
    enum StyleInput {
        Anonymous(LitStr),
        Named(Ident, LitStr, bool),
    }

    impl Parse for StyleInput {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            if input.peek(Ident) && input.peek2(Token![,]) {
                let name: Ident = input.parse()?;
                let _: Token![,] = input.parse()?;
                let path: LitStr = input.parse()?;
                let is_global = if input.peek(Token![,]) && input.peek2(kw::global) {
                    let _: Token![,] = input.parse()?;
                    let _: kw::global = input.parse()?;
                    true
                } else {
                    false
                };
                Ok(StyleInput::Named(name, path, is_global))
            } else {
                Ok(StyleInput::Anonymous(input.parse()?))
            }
        }
    }

    let parsed_input = parse_macro_input!(input as StyleInput);
    let (namespace, css_lit, is_global) = match parsed_input {
        StyleInput::Anonymous(lit) => (None, lit, false),
        StyleInput::Named(ident, lit, global) => (Some(ident), lit, global),
    };

    let processed = match resolve_and_process_style(&css_lit.value(), css_lit.span(), is_global, &[]) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into()
    };

    let scope_hash = processed.scope_hash;
    let scoped_css = processed.scoped_css;
    let class_constants = processed.class_constants;
    let include_quote = generate_include_quote(processed.include_path);

    if let Some(mod_name) = namespace {
        let namespace_module = generate_namespace_module(
            &mod_name,
            &scope_hash,
            &scoped_css,
            &class_constants,
            &include_quote,
        );
        let expanded = quote! {
            #namespace_module
            let _ = #mod_name::get_scope();
        };
        TokenStream::from(expanded)
    } else {
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
    let candidates = vec![
        base.join(file_path),
        base.join("src").join(file_path),
        base.join("styles").join(file_path),
        base.join("assets").join(file_path),
        base.join("src/styles").join(file_path),
        base.join("src/assets").join(file_path),
    ];
    for path in &candidates {
        if path.exists() && path.is_file() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
            return Ok((path.clone(), content));
        }
    }
    let tried_paths = candidates.iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "Style file '{}' not found.\nSearched in:\n{}\n",
        file_path,
        tried_paths
    ))
}

fn is_likely_file_path(input: &str) -> bool {
    input.ends_with(".css") || input.ends_with(".scss") || input.ends_with(".sass")
        || input.contains('/') || input.contains('\\')
}
