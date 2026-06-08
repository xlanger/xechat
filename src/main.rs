//! XEChat 桌面应用入口
//!
//! 配置 Dioxus 桌面窗口（标题、尺寸、透明标题栏）并启动应用。

#![allow(unexpected_cfgs)]

use dioxus::desktop::{Config, WindowBuilder};
#[cfg(target_os = "macos")]
use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

fn main() {
    let icon = {
        use image::io::Reader as ImageReader;
        use std::io::Cursor;
        let data = include_bytes!("../assets/icon.png");
        let cursor = Cursor::new(data);

        ImageReader::new(cursor)
            .with_guessed_format()
            .expect("无法识别图标格式")
            .decode()
            .expect("无法解码图标")
            .to_rgba8()
    };
    let dioxus_icon = dioxus::desktop::tao::window::Icon::from_rgba(
        icon.as_raw().to_vec(),
        icon.width(),
        icon.height(),
    )
    .expect("无法创建图标");

    let mut window_builder = WindowBuilder::new()
        .with_title("")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1144, 800))
        .with_min_inner_size(dioxus::desktop::LogicalSize::new(1300, 680))
        .with_window_icon(Some(dioxus_icon.clone()))
        .with_transparent(true)
        .with_decorations(true);

    #[cfg(target_os = "macos")]
    {
        window_builder = window_builder
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true);
    }

    let custom_head = format!(
        "<style>{}</style>",
        include_str!(concat!(env!("OUT_DIR"), "/global.css"))
    );

    let config = Config::new()
        .with_icon(dioxus_icon.clone())
        .with_window(window_builder)
        .with_custom_head(custom_head);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(xechat::app::App);
}
