#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dioxus::desktop::{wry, Config as DesktopConfig, WindowBuilder};
use dioxus::prelude::*;
use std::borrow::Cow;

mod app;
mod art;
mod components;
mod icons;
mod modutil;
mod picker;
mod state;
mod themes;
mod views;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        let app_data = yago_app::default_app_data_dir();
        let webview_dir = app_data.join("webview");
        let _ = std::fs::create_dir_all(&webview_dir);

        let init_result = yago_app::init().await.map_err(|e| e.to_string());

        LaunchBuilder::new()
            .with_cfg(desktop! {
                DesktopConfig::new()
                    .with_data_directory(webview_dir)
                    .with_window(
                        WindowBuilder::new()
                            .with_title("YAGO")
                            .with_decorations(false)
                            .with_transparent(false)
                            .with_inner_size(dioxus::desktop::LogicalSize::new(1440.0, 900.0))
                            .with_min_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 680.0)),
                    )
                    .with_background_color((11, 14, 20, 255))
                    .with_custom_head(
                        r#"<style>html, body { background-color: #0b0e14 !important; color: #e6ebf4; margin: 0; padding: 0; user-select: none; }</style>"#
                            .to_string(),
                    )
                    .with_disable_context_menu(true)
                    .with_custom_protocol("yago-asset", |_id, request| {
                        let reply = yago_app::assets::serve_asset(request.uri().path());
                        wry::http::Response::builder()
                            .status(reply.status)
                            .header("Content-Type", reply.mime)
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Cow::Owned(reply.body))
                            .unwrap()
                    })
            })
            .with_context(init_result)
            .launch(app::App);
    });
}
