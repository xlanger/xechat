//! CSS class helper type for combining multiple classes
//!
//! This module provides a `CssClass` type that allows combining multiple
//! CSS class names using the `+` operator.

use std::fmt;
use dioxus_core::{IntoAttributeValue, AttributeValue};

/// A CSS class or collection of classes.
///
/// This type allows combining multiple CSS classes using the `+` operator:
/// ```rust,ignore
/// let classes = css::btn + css::primary;
/// // Renders as "btn primary"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]

pub enum CssClass {
    Static(&'static str),
    Dynamic(Vec<String>),
}

impl CssClass {
    /// Create a new CssClass from a static string (for const usage)
    pub const fn new(class: &'static str) -> Self {
        CssClass::Static(class)
    }

    /// Normalizes a CssClass into a Vec<String>.
    /// Static variants become a single-element vector.
    #[inline]
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Static(s) => vec![s.to_string()],
            Self::Dynamic(v) => v,
        }
    }

    /// Prepends a static class string to a dynamic vector of classes.
    #[inline]
    fn merge_static_into_dynamic(s: &'static str, v: Vec<String>) -> CssClass {
        let mut result = vec![s.to_string()];
        let mut v = v;
        result.append(&mut v);
        CssClass::Dynamic(result)
    }

    /// Appends v2 to v1, returning a merged dynamic CssClass.
    #[inline]
    fn merge_two_dynamics(v1: Vec<String>, v2: Vec<String>) -> CssClass {
        let mut v1 = v1;
        let mut v2 = v2;
        v1.append(&mut v2);
        CssClass::Dynamic(v1)
    }
}

impl IntoAttributeValue for CssClass {
    fn into_value(self) -> AttributeValue {
        AttributeValue::Text(self.to_string())
    }
}

impl fmt::Display for CssClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(s) => write!(f, "{}", s),
            Self::Dynamic(classes) => write!(f, "{}", classes.join(" ")),
        }
    }
}

impl From<&str> for CssClass {
    fn from(s: &str) -> Self {
        // We can't safely assume &str is static here without unsafe,
        // so we convert to string and use Dynamic.
        // For const usage, use CssClass::new()
        CssClass::Dynamic(vec![s.to_string()])
    }
}

impl From<String> for CssClass {
    fn from(s: String) -> Self {
        CssClass::Dynamic(vec![s])
    }
}

// Implement Add trait to combine classes
impl std::ops::Add for CssClass {
    type Output = CssClass;

    fn add(self, other: CssClass) -> CssClass {
        let v2 = other.into_vec();
        match self {
            Self::Static(s1) => Self::merge_static_into_dynamic(s1, v2),
            Self::Dynamic(v1) => Self::merge_two_dynamics(v1, v2),
        }
    }
}

// Support &CssClass + &CssClass
impl std::ops::Add for &CssClass {
    type Output = CssClass;

    fn add(self, other: &CssClass) -> CssClass {
        self.clone() + other.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_core::AttributeValue;

    #[test]
    fn new_creates_static_variant() {
        let css = CssClass::new("btn");
        assert_eq!(css, CssClass::Static("btn"));
    }

    #[test]
    fn from_str_creates_dynamic_with_one_element() {
        let css = CssClass::from("btn");
        assert_eq!(css, CssClass::Dynamic(vec!["btn".to_string()]));
    }

    #[test]
    fn from_string_creates_dynamic_with_one_element() {
        let css = CssClass::from("btn".to_string());
        assert_eq!(css, CssClass::Dynamic(vec!["btn".to_string()]));
    }

    #[test]
    fn add_static_static_creates_dynamic_with_two_elements() {
        let a = CssClass::new("btn");
        let b = CssClass::new("primary");
        let result = a + b;
        assert_eq!(
            result,
            CssClass::Dynamic(vec!["btn".to_string(), "primary".to_string()])
        );
    }

    #[test]
    fn add_static_dynamic_merges_elements() {
        let a = CssClass::new("btn");
        let b = CssClass::Dynamic(vec!["primary".to_string(), "large".to_string()]);
        let result = a + b;
        assert_eq!(
            result,
            CssClass::Dynamic(vec![
                "btn".to_string(),
                "primary".to_string(),
                "large".to_string()
            ])
        );
    }

    #[test]
    fn add_dynamic_static_appends_element() {
        let a = CssClass::Dynamic(vec!["btn".to_string(), "primary".to_string()]);
        let b = CssClass::new("large");
        let result = a + b;
        assert_eq!(
            result,
            CssClass::Dynamic(vec![
                "btn".to_string(),
                "primary".to_string(),
                "large".to_string()
            ])
        );
    }

    #[test]
    fn add_dynamic_dynamic_merges_all_elements() {
        let a = CssClass::Dynamic(vec!["btn".to_string(), "primary".to_string()]);
        let b = CssClass::Dynamic(vec!["large".to_string(), "active".to_string()]);
        let result = a + b;
        assert_eq!(
            result,
            CssClass::Dynamic(vec![
                "btn".to_string(),
                "primary".to_string(),
                "large".to_string(),
                "active".to_string()
            ])
        );
    }

    #[test]
    fn add_references_works_via_clone() {
        let a = CssClass::new("btn");
        let b = CssClass::new("primary");
        let result = &a + &b;
        assert_eq!(
            result,
            CssClass::Dynamic(vec!["btn".to_string(), "primary".to_string()])
        );
        // Originals should remain unchanged
        assert_eq!(a, CssClass::Static("btn"));
        assert_eq!(b, CssClass::Static("primary"));
    }

    #[test]
    fn display_static_returns_just_the_string() {
        let css = CssClass::new("btn");
        assert_eq!(format!("{}", css), "btn");
    }

    #[test]
    fn display_dynamic_returns_space_joined_classes() {
        let css = CssClass::Dynamic(vec![
            "btn".to_string(),
            "primary".to_string(),
            "large".to_string(),
        ]);
        assert_eq!(format!("{}", css), "btn primary large");
    }

    #[test]
    fn into_value_static_returns_text_with_string_representation() {
        let css = CssClass::new("btn");
        let value = css.into_value();
        match value {
            AttributeValue::Text(s) => assert_eq!(s, "btn"),
            _ => panic!("Expected AttributeValue::Text variant"),
        }
    }

    #[test]
    fn into_value_dynamic_returns_text_with_joined_string() {
        let css = CssClass::Dynamic(vec!["btn".to_string(), "primary".to_string()]);
        let value = css.into_value();
        match value {
            AttributeValue::Text(s) => assert_eq!(s, "btn primary"),
            _ => panic!("Expected AttributeValue::Text variant"),
        }
    }
}
