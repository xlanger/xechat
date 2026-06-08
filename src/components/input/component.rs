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
                        // 过滤非法字符，只保留数字、小数点、负号
                        let filtered = filter_number_input(&new_value);

                        // 合法数字实时边界截断
                        let clamped = if is_valid_number(&filtered) {
                            clamp_number_value(&filtered, min, max)
                        } else {
                            filtered
                        };

                        edit_value.set(clamped.clone());

                        // 合法数字才更新 last_valid_value
                        if is_valid_number(&clamped) {
                            last_valid_value.set(clamped.clone());
                        }
                        // 空字符串不触发 on_input
                        if !clamped.is_empty() {
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

                        // 空字符串或非数字：回退到上一次合法值
                        let fallback = if current.is_empty() || !is_valid_number(&current) {
                            last_valid_value.read().clone()
                        } else {
                            current
                        };

                        // 去除末尾小数点，然后边界截断
                        let normalized = normalize_number(&fallback);
                        let clamped = clamp_number_value(&normalized, min, max);

                        // 截断后为空（如 last_valid_value 也是空），则回退到 min 或 0
                        let final_value = if clamped.is_empty() {
                            min.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "0".to_string())
                        } else {
                            clamped
                        };

                        if final_value != *edit_value.read() {
                            edit_value.set(final_value.clone());
                            last_valid_value.set(final_value.clone());
                            on_input.call(final_value);
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
        match c {
            '-' if i == 0 => result.push(c),
            '.' if !has_dot => {
                has_dot = true;
                result.push(c);
            }
            c if c.is_ascii_digit() => result.push(c),
            _ => {}
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

/// 对数字输入值进行边界截断。
pub fn clamp_number_value(value: &str, min: Option<f64>, max: Option<f64>) -> String {
    if value.is_empty() {
        return value.to_string();
    }

    if let Ok(num) = value.parse::<f64>() {
        let mut clamped = num;
        if let Some(min_val) = min
            && clamped < min_val {
                clamped = min_val;
            }
        if let Some(max_val) = max
            && clamped > max_val {
                clamped = max_val;
            }
        if clamped != num {
            if num.fract() == 0.0 && clamped.fract() == 0.0 {
                return format!("{:.0}", clamped);
            }
            return clamped.to_string();
        }
    }

    value.to_string()
}

