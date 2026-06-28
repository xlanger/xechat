use xechat::platform::SystemTheme;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

/// 模拟 windows 模块的 parse_reg_theme_output 函数。
/// 由于 windows 模块仅在 target_os = "windows" 下编译，
/// 这里直接复制函数逻辑进行测试。
fn parse_reg_theme_output(output: Result<std::process::Output, std::io::Error>) -> SystemTheme {
    let Ok(out) = output else { return SystemTheme::Dark };
    if !out.status.success() {
        return SystemTheme::Dark;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("0x1") {
        SystemTheme::Light
    } else {
        SystemTheme::Dark
    }
}

// ── parse_reg_theme_output ──────────────────────────────────────

#[test]
fn test_parse_reg_theme_output_light() {
    let output = std::process::Output {
        status: ExitStatus::from_raw(0),
        stdout: b"AppsUseLightTheme    REG_DWORD    0x1".to_vec(),
        stderr: vec![],
    };
    assert_eq!(parse_reg_theme_output(Ok(output)), SystemTheme::Light);
}

#[test]
fn test_parse_reg_theme_output_dark() {
    let output = std::process::Output {
        status: ExitStatus::from_raw(0),
        stdout: b"AppsUseLightTheme    REG_DWORD    0x0".to_vec(),
        stderr: vec![],
    };
    assert_eq!(parse_reg_theme_output(Ok(output)), SystemTheme::Dark);
}

#[test]
fn test_parse_reg_theme_output_failed_command() {
    let output = std::process::Output {
        status: ExitStatus::from_raw(1),
        stdout: vec![],
        stderr: b"ERROR".to_vec(),
    };
    assert_eq!(parse_reg_theme_output(Ok(output)), SystemTheme::Dark);
}

#[test]
fn test_parse_reg_theme_output_io_error() {
    let err = std::io::Error::new(std::io::ErrorKind::NotFound, "reg not found");
    assert_eq!(parse_reg_theme_output(Err(err)), SystemTheme::Dark);
}

#[test]
fn test_parse_reg_theme_output_unrecognized_value() {
    let output = std::process::Output {
        status: ExitStatus::from_raw(0),
        stdout: b"AppsUseLightTheme    REG_DWORD    0x2".to_vec(),
        stderr: vec![],
    };
    assert_eq!(parse_reg_theme_output(Ok(output)), SystemTheme::Dark);
}
