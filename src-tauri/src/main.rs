// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMA-BUF renderer leaves the window blank where GPU buffers
    // cannot be shared: virtual machines without 3D acceleration and some
    // NVIDIA drivers. Found on an Ubuntu 24.04 GNOME VM (2026-11-06). Turn it
    // off unless the person set the variable themselves. Set before any
    // thread starts, so changing the environment is safe here.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    freeboard_lib::run();
}
