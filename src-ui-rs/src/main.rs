#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dioxus::desktop::{wry, Config as DesktopConfig, WindowBuilder};
use dioxus::prelude::*;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;

mod app;
mod art;
mod components;
mod icons;
mod modutil;
mod picker;
mod state;
mod themes;
mod views;

pub type DeepLinkReceiver = Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<String>>>;

fn main() {
    let app_data = yago_app::default_app_data_dir();
    let _log_guard = yago_app::init_logging(&app_data);
    yago_app::install_panic_hook(app_data.clone());

    let deep_link = std::env::args().find(|a| a.starts_with("yago://"));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        let mut instance_guard = match yago_app::acquire_or_forward(&app_data, deep_link.as_deref()).await {
            yago_app::InstanceResult::ForwardedToExisting => {
                eprintln!("Forwarded request to existing YAGO instance. Exiting.");
                return;
            }
            yago_app::InstanceResult::Primary(guard) => guard,
        };

        let raw_rx = instance_guard
            .take_receiver()
            .unwrap_or_else(|| tokio::sync::mpsc::unbounded_channel().1);
        let deep_link_rx: DeepLinkReceiver = Arc::new(Mutex::new(raw_rx));

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
            .with_context(deep_link_rx)
            .launch(app::App);

        // Keep instance guard in scope until window closes
        drop(instance_guard);
    });
}
