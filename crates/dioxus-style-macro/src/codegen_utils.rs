//! Helpers for generating scoped CSS class names and token streams

use quote::quote;
use proc_macro2::TokenStream;
use syn::Ident;

use crate::class_extractor::SelectorInfo;

pub fn generate_class_constant(
    selector_info: &SelectorInfo,
    scope_hash: &str,
    is_global: bool,
) -> TokenStream {
    let rust_ident = selector_info.name();
    let ident = Ident::new(rust_ident, proc_macro2::Span::call_site());

    match selector_info {
        SelectorInfo::Id(id_name) => {
            let actual_id = id_name.replace('_', "-");
            quote! {
                #[allow(non_upper_case_globals)]
                pub const #ident: &'static str = #actual_id;
            }
        }
        SelectorInfo::Element(_) | SelectorInfo::Pseudo(_) | SelectorInfo::Class(_) => {
            let css_class_hyphen = rust_ident.replace('_', "-");
            let class_name = if is_global {
                css_class_hyphen
            } else {
                format!("{}_{}", scope_hash, css_class_hyphen)
            };
            quote! {
                #[allow(non_upper_case_globals)]
                pub const #ident: ::dioxus_style::CssClass = ::dioxus_style::CssClass::new(#class_name);
            }
        }
    }
}

pub fn generate_namespace_module(
    namespace_name: &Ident,
    _scope_hash: &str,
    scoped_css: &str,
    class_constants: &[TokenStream],
    include_quote: &TokenStream,
) -> TokenStream {
    quote! {
        mod #namespace_name {
            pub fn get_style_content() -> &'static str {
                #include_quote
                #scoped_css
            }

            #(#class_constants)*
        }
    }
}

pub fn generate_include_quote(include_path: Option<String>) -> TokenStream {
    match include_path {
        Some(p) => quote! { let _css_tracker = include_str!(#p); },
        None => quote! {},
    }
}
