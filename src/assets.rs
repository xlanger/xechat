/// 获取内嵌 Logo 图片的 base64 Data URL
pub fn logo_data_url() -> String {
    let bytes = include_bytes!("../assets/icon.png");
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    format!("data:image/png;base64,{}", b64)
}
