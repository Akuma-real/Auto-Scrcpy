//! scrcpy 智能启动器
//! 自动下载最新版本的scrcpy并智能管理设备连接

mod single_instance;
mod github_api;
mod downloader;
mod device_monitor;
mod launcher;
mod ui;

use single_instance::SingleInstanceGuard;
use launcher::ScrcpyLauncher;
use ui::TerminalUI;

#[tokio::main]
async fn main() {
    // 显示美化的标题
    TerminalUI::print_header();

    // 在任何异步操作之前进行单实例检查
    #[cfg(target_os = "windows")]
    let _guard = match SingleInstanceGuard::new() {
        Ok(guard) => guard,
        Err(e) => {
            TerminalUI::print_error_panel("单实例检查失败", &e.to_string());
            TerminalUI::wait_for_key();
            return;
        }
    };

    #[cfg(not(target_os = "windows"))]
    let _guard = match SingleInstanceGuard::new() {
        Ok(guard) => guard,
        Err(e) => {
            TerminalUI::print_error_panel("单实例检查失败", &e.to_string());
            TerminalUI::wait_for_key();
            return;
        }
    };

    // 初始化启动器
    let mut launcher = match ScrcpyLauncher::new().await {
        Ok(launcher) => launcher,
        Err(e) => {
            TerminalUI::print_error_panel("启动器初始化失败", &e.to_string());
            TerminalUI::wait_for_key();
            return;
        }
    };

    // 设置Ctrl+C处理
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
        let _ = tx.send(()).await;
    });

    tokio::select! {
        _ = launcher.run() => {
            // 正常结束（实际上不会到这里，因为run是无限循环）
        }
        _ = rx.recv() => {
            TerminalUI::print_stop("收到退出信号，正在关闭...");
            TerminalUI::print_goodbye();
        }
    }
}