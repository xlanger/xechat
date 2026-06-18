use dioxus::prelude::*;
use dioxus_style::with_css;

/// 输入模式枚举。
///
/// 定义输入框支持的三种输入类型：普通文本、密码掩码和数字限制。
#[derive(Clone, Copy, PartialEq, Default)]
pub enum InputType {
    /// 普通文本输入（默认）。
    #[default]
    Text,
    /// 密码输入，字符显示为掩码。
    Password,
    /// 数字输入，限制只能输入数字字符。
    Number,
}

/// 处理数字输入的 oninput 逻辑：过滤、截断、更新编辑值。
///
/// 返回 `(edit_value, last_valid_value, should_call_on_input)` 元组。
#[inline]
pub fn handle_number_input(
    new_value: &str,
    min: Option<f64>,
    max: Option<f64>,
) -> (String, Option<String>, bool) {
    let filtered = filter_number_input(new_value);
    let clamped = if is_valid_number(&filtered) {
        clamp_number_value(&filtered, min, max)
    } else {
        filtered
    };

    let updated_valid = if is_valid_number(&clamped) {
        Some(clamped.clone())
    } else {
        None
    };

    let should_call = !clamped.is_empty();
    (clamped, updated_valid, should_call)
}

/// 处理数字输入的 blur 逻辑：回退、规范化、截断。
///
/// 返回最终的 `(edit_value, last_valid_value)` 元组，仅在值变化时返回 `Some`。
#[inline]
pub fn handle_number_blur(
    current: &str,
    last_valid: &str,
    min: Option<f64>,
    max: Option<f64>,
) -> Option<(String, String)> {
    let fallback = if current.is_empty() || !is_valid_number(current) {
        last_valid.to_string()
    } else {
        current.to_string()
    };

    let normalized = normalize_number(&fallback);
    let clamped = clamp_number_value(&normalized, min, max);

    let final_value = if clamped.is_empty() {
        min.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "0".to_string())
    } else {
        clamped
    };

    if final_value != current {
        Some((final_value.clone(), final_value))
    } else {
        None
    }
}

#[with_css(css, "styles/components/input.scss")]
/// 通用输入框组件。
///
/// 支持文本、密码、数字三种输入模式，
/// 通过 `on_input` 回调将值变化传递给父组件实现受控模式。
///
/// # 示例
///
/// ```ignore
/// rsx! {
///     Input {
///         value: state.read().clone(),
///         placeholder: "请输入内容",
///         input_type: InputType::Text,
///         on_input: move |v| state.set(v),
///     }
/// }
/// ```
#[component]
pub fn Input(
    /// 当前值（受控模式）
    value: String,
    /// 占位提示文字
    placeholder: String,
    /// 输入模式（text/password/number）
    #[props(default)]
    input_type: InputType,
    /// 最小值（仅 Number 类型有效）
    #[props(default)]
    min: Option<f64>,
    /// 最大值（仅 Number 类型有效）
    #[props(default)]
    max: Option<f64>,
    /// 右侧图标（可选），传入 Element
    #[props(default)]
    right_icon: Option<Element>,
    /// 值变化回调
    on_input: EventHandler<String>,
) -> Element {
    let input_type_str = match input_type {
        InputType::Text => "text",
        InputType::Password => "password",
        // 使用 text 而非 number，避免浏览器对不完整数字（如 "0."）返回空字符串
        InputType::Number => "text",
    };

    // 本地编辑值，用于在输入过程中保持光标位置和中间状态
    // 避免受控模式下父组件 value 覆盖输入中的状态
    let mut edit_value = use_signal(|| value.clone());
    // 记录上一次的合法值，用于 blur 时回退
    let mut last_valid_value = use_signal(|| value.clone());

    // 当外部 value 变化时同步（初始化或父组件主动修改时）
    let value_for_effect = value.clone();
    use_effect(move || {
        edit_value.set(value_for_effect.clone());
        last_valid_value.set(value_for_effect.clone());
    });

    let input_class = if right_icon.is_some() {
        format!("{} has-right-icon", css::input_field)
    } else {
        css::input_field.to_string()
    };

    rsx! {
        div {
            class: "{css::input_wrapper}",
            input {
                class: "{input_class}",
                r#type: "{input_type_str}",
                // 使用本地 edit_value 作为真实显示值，避免父组件 value 覆盖输入过程
                value: "{edit_value}",
                placeholder: "{placeholder}",
                oninput: move |event| {
                    let new_value = event.value();

                    if input_type == InputType::Number {
                        let (clamped, updated_valid, should_call) = handle_number_input(&new_value, min, max);
                        edit_value.set(clamped.clone());
                        if let Some(valid) = updated_valid {
                            last_valid_value.set(valid);
                        }
                        if should_call {
                            on_input.call(clamped);
                        }
                    } else {
                        edit_value.set(new_value.clone());
                        on_input.call(new_value);
                    }
                },
                onfocusout: move |_| {
                    if input_type == InputType::Number {
                        let current = edit_value.read().clone();
                        let last = last_valid_value.read().clone();
                        if let Some((new_edit, new_valid)) = handle_number_blur(&current, &last, min, max) {
                            edit_value.set(new_edit.clone());
                            last_valid_value.set(new_valid);
                            on_input.call(new_edit);
                        }
                    }
                },
            }
            if let Some(icon) = right_icon.as_ref() {
                div {
                    class: "{css::input_right_icon}",
                    {icon}
                }
            }
        }
    }
}

/// 判断字符是否为合法数字输入字符。
///
/// 负号仅在首位合法，小数点仅当尚未出现时合法，数字始终合法。
fn is_valid_number_char(c: char, index: usize, has_dot: bool) -> bool {
    match c {
        '-' if index == 0 => true,
        '.' if !has_dot => true,
        c if c.is_ascii_digit() => true,
        _ => false,
    }
}

/// 过滤输入字符串，只保留合法数字字符。
///
/// 规则：
/// - 只允许数字、小数点、负号
/// - 负号只能在开头
/// - 小数点只能出现一次
pub fn filter_number_input(input: &str) -> String {
    let mut result = String::new();
    let mut has_dot = false;

    for (i, c) in input.chars().enumerate() {
        if is_valid_number_char(c, i, has_dot) {
            if c == '.' {
                has_dot = true;
            }
            result.push(c);
        }
    }

    result
}

/// 检查字符串是否为完整合法数字（可解析为 f64）。
pub fn is_valid_number(value: &str) -> bool {
    if value.is_empty() || value == "-" || value == "." || value == "-." {
        return false;
    }
    value.parse::<f64>().is_ok()
}

/// 规范化数字字符串，去除末尾多余的小数点。
///
/// "1." -> "1", "0." -> "0", "1.5" -> "1.5"
pub fn normalize_number(value: &str) -> String {
    if value.ends_with('.') && value != "." {
        return value.trim_end_matches('.').to_string();
    }
    value.to_string()
}

/// 解析字符串为 f64，失败返回 None。
fn parse_clamped_value(value: &str) -> Option<f64> {
    if value.is_empty() {
        return None;
    }
    value.parse::<f64>().ok()
}

/// 对数值应用边界截断，返回截断后的值和是否发生了截断。
fn apply_clamp_bounds(num: f64, min: Option<f64>, max: Option<f64>) -> (f64, bool) {
    let mut clamped = num;
    if let Some(min_val) = min
        && clamped < min_val {
            clamped = min_val;
        }
    if let Some(max_val) = max
        && clamped > max_val {
            clamped = max_val;
        }
    (clamped, clamped != num)
}

/// 格式化截断后的数值，整数时省略小数点。
fn format_clamped_value(clamped: f64, original: f64) -> String {
    if original.fract() == 0.0 && clamped.fract() == 0.0 {
        format!("{:.0}", clamped)
    } else {
        clamped.to_string()
    }
}

/// 对数字输入值进行边界截断。
pub fn clamp_number_value(value: &str, min: Option<f64>, max: Option<f64>) -> String {
    let Some(num) = parse_clamped_value(value) else {
        return value.to_string();
    };

    let (clamped, was_clamped) = apply_clamp_bounds(num, min, max);
    if was_clamped {
        format_clamped_value(clamped, num)
    } else {
        value.to_string()
    }
}

