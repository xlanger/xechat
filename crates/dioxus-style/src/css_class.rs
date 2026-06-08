//! CSS class helper type for combining multiple classes
//!
//! This module provides a `CssClass` type that allows combining multiple
//! CSS class names using the `+` operator.

use std::fmt;
use dioxus_core::{IntoAttributeValue, AttributeValue};

/// A CSS class or collection of classes.
///
/// This type allows combining multiple CSS classes using the `+` operator:
/// ```rust
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
        match (self, other) {
            (Self::Static(s1), Self::Static(s2)) => {
                CssClass::Dynamic(vec![s1.to_string(), s2.to_string()])
            }
            (Self::Static(s1), Self::Dynamic(mut v2)) => {
                let mut v = vec![s1.to_string()];
                v.append(&mut v2);
                CssClass::Dynamic(v)
            }
            (Self::Dynamic(mut v1), Self::Static(s2)) => {
                v1.push(s2.to_string());
                CssClass::Dynamic(v1)
            }
            (Self::Dynamic(mut v1), Self::Dynamic(mut v2)) => {
                v1.append(&mut v2);
                CssClass::Dynamic(v1)
            }
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
