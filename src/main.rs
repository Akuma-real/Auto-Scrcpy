//! scrcpy 智能启动器
//! 自动下载最新版本的scrcpy并智能管理设备连接

mod single_instance;
mod github_api;
mod downloader;
mod device_monitor;
mod launcher;

use single_instance::SingleInstanceGuard;
use launcher::ScrcpyLauncher;

#[tokio::main]
async fn main() {
    println!("🚀 scrcpy 智能启动器 v{}", env!("CARGO_PKG_VERSION"));
    println!("🌐 支持自动下载最新版本");
    println!("🔒 单实例运行保护");
    println!("========================================");

    // 在任何异步操作之前进行单实例检查
    #[cfg(target_os = "windows")]
    let _guard = match SingleInstanceGuard::new() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("❌ 单实例检查失败: {}", e);
            return;
        }
    };

    #[cfg(not(target_os = "windows"))]
    let _guard = match SingleInstanceGuard::new() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("❌ 单实例检查失败: {}", e);
            return;
        }
    };

    // 初始化启动器
    let mut launcher = match ScrcpyLauncher::new().await {
        Ok(launcher) => launcher,
        Err(e) => {
            eprintln!("❌ 初始化失败: {}", e);
            println!("按任意键退出...");
            let _ = std::io::stdin().read_line(&mut String::new());
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
            println!("\n🛑 收到退出信号，正在关闭...");
            println!("👋 再见！");
        }
    }
}