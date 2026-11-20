#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod server;

use server::{Compression, Player, Snapshot};
use tauri::{AppHandle, Manager, State};

fn launch_browser(url: &str) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|error| format!("无法打开系统浏览器，请复制访问地址手动打开：{error}"))
}

#[tauri::command]
async fn open_project(state: State<'_, Player>) -> Result<(), String> {
    let snapshot = server::project_state(state).await?;
    let url = snapshot.url.ok_or("请先启动项目。")?;
    launch_browser(&url)
}

#[tauri::command]
async fn open_demo(app: AppHandle, state: State<'_, Player>) -> Result<Snapshot, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|err| err.to_string())?
        .join(concat!("webgl-diagnostic-", env!("CARGO_PKG_VERSION")));
    tokio::fs::create_dir_all(root.join("Build"))
        .await
        .map_err(|err| format!("无法创建演示目录：{err}"))?;
    tokio::fs::write(
        root.join("index.html"),
        include_bytes!("../demo/index.html"),
    )
    .await
    .map_err(|err| format!("无法写入演示页面：{err}"))?;
    tokio::fs::write(
        root.join("Build/diagnostic.js"),
        include_bytes!("../demo/diagnostic.js"),
    )
    .await
    .map_err(|err| format!("无法写入演示脚本：{err}"))?;
    let result = server::start_project(
        root.to_string_lossy().into_owned(),
        Compression::Auto,
        true,
        None,
        state,
    )
    .await?;
    if let Some(url) = result.url.as_deref() {
        launch_browser(url)?;
    }
    Ok(result)
}

#[cfg(windows)]
fn apply_title_bar_color(app: &tauri::App) {
    use std::ffi::c_void;

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            window: *mut c_void,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }

    const DWMWA_CAPTION_COLOR: u32 = 35;
    // COLORREF stores bytes in BGR order. Match the neutral workbench chrome (#fafafd).
    const CAPTION_COLOR: u32 = 0x00fd_fafa;
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(handle) = window.hwnd() else { return };
    // The HWND belongs to the live Tauri window and DWM reads one u32 from the pointer.
    // Systems that do not support this attribute keep the default native title bar.
    unsafe {
        DwmSetWindowAttribute(
            handle.0,
            DWMWA_CAPTION_COLOR,
            (&CAPTION_COLOR as *const u32).cast(),
            4,
        );
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(windows)]
            apply_title_bar_color(app);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .manage(Player::default())
        .invoke_handler(tauri::generate_handler![
            catalog::scan_projects,
            server::check_recent_projects,
            server::start_project,
            server::project_state,
            server::stop_project,
            open_project,
            open_demo
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 Unity WebGL 启动器");
}
