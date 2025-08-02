//! 终端UI模块
//! 提供美观的终端界面和用户交互功能

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// 终端UI管理器
pub struct TerminalUI;

impl TerminalUI {
    /// 清屏
    pub fn clear_screen() {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(&["/c", "cls"])
                .status();
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            print!("\x1B[2J\x1B[1;1H");
            let _ = io::stdout().flush();
        }
    }

    /// 打印应用标题和版本信息
    pub fn print_header() {
        Self::clear_screen();
        
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                    🚀 SCRCPY 智能启动器                      ║");
        println!("║                        v{}                           ║", env!("CARGO_PKG_VERSION"));
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  🌐 自动下载最新版本  │  🔒 单实例运行保护  │  📱 智能监控    ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();
    }

    /// 打印分隔线
    pub fn print_separator() {
        println!("────────────────────────────────────────────────────────────────");
    }

    /// 打印带时间戳的状态信息
    pub fn print_status(icon: &str, message: &str) {
        let timestamp = Self::get_timestamp();
        println!("[{}] {} {}", timestamp, icon, message);
    }

    /// 打印成功信息
    pub fn print_success(message: &str) {
        Self::print_status("✅", message);
    }

    /// 打印错误信息
    pub fn print_error(message: &str) {
        Self::print_status("❌", message);
    }

    /// 打印警告信息
    pub fn print_warning(message: &str) {
        Self::print_status("⚠️", message);
    }

    /// 打印信息
    pub fn print_info(message: &str) {
        Self::print_status("ℹ️", message);
    }

    /// 打印进度信息
    pub fn print_progress(message: &str) {
        Self::print_status("📊", message);
    }

    /// 打印设备相关信息
    pub fn print_device(message: &str) {
        Self::print_status("📱", message);
    }

    /// 打印下载相关信息
    pub fn print_download(message: &str) {
        Self::print_status("📥", message);
    }

    /// 打印文件相关信息
    pub fn print_file(message: &str) {
        Self::print_status("📁", message);
    }

    /// 打印版本相关信息
    pub fn print_version(message: &str) {
        Self::print_status("📦", message);
    }

    /// 打印网络相关信息
    pub fn print_network(message: &str) {
        Self::print_status("🌐", message);
    }

    /// 打印启动相关信息
    pub fn print_launch(message: &str) {
        Self::print_status("🚀", message);
    }

    /// 打印停止相关信息
    pub fn print_stop(message: &str) {
        Self::print_status("🛑", message);
    }

    /// 打印提示信息
    pub fn print_tip(message: &str) {
        Self::print_status("💡", message);
    }

    /// 打印搜索相关信息
    pub fn print_search(message: &str) {
        Self::print_status("🔍", message);
    }

    /// 打印锁定相关信息
    pub fn print_lock(message: &str) {
        Self::print_status("🔒", message);
    }

    /// 获取当前时间戳
    fn get_timestamp() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();
        let secs = now.as_secs();
        let hours = (secs / 3600) % 24;
        let minutes = (secs / 60) % 60;
        let seconds = secs % 60;
        format!("{:02}:{:02}:{:02}", hours + 8, minutes, seconds) // UTC+8
    }

    /// 打印美化的进度条
    pub fn print_progress_bar(progress: u32, downloaded_mb: f64, total_mb: f64) -> Result<(), Box<dyn std::error::Error>> {
        let bar_length = 40; // 40个字符的进度条
        let filled_length = (progress as f64 / 100.0 * bar_length as f64) as usize;
        
        let mut bar = String::new();
        for i in 0..bar_length {
            if i < filled_length {
                bar.push('█');
            } else if i == filled_length && progress < 100 {
                bar.push('▌');
            } else {
                bar.push('░');
            }
        }
        
        print!("\r📊 下载进度: [{}] {:3.1}% ({:.2} MB / {:.2} MB)", 
               bar, progress as f64, downloaded_mb, total_mb);
        io::stdout().flush()?;
        Ok(())
    }

    /// 询问用户确认
    pub fn ask_confirmation(message: &str) -> bool {
        print!("❓ {} ", message);
        Self::print_input_prompt();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim().to_lowercase();
                input == "y" || input == "yes" || input == "是" || input == "确定"
            }
            Err(_) => false,
        }
    }

    /// 打印输入提示
    fn print_input_prompt() {
        print!("(y/N): ");
        let _ = io::stdout().flush();
    }

    /// 打印监控状态面板
    pub fn print_monitor_panel() {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                      📱 设备监控面板                         ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  状态: 正在监控设备连接...                                   ║");
        println!("║  提示: 连接Android设备后将自动启动scrcpy                     ║");
        println!("║  操作: 按 Ctrl+C 退出程序                                   ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        Self::print_separator();
    }

    /// 打印版本信息对比
    pub fn print_version_comparison(local: &str, remote: &str) {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                      📦 版本信息对比                         ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  本地版本: {:45} ║", local);
        println!("║  远程版本: {:45} ║", remote);
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    /// 打印下载信息面板
    pub fn print_download_panel(filename: &str, size_mb: f64) {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                      📥 下载信息                             ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  文件名: {:49} ║", filename);
        println!("║  大小:   {:.2} MB{:42} ║", size_mb, "");
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    /// 打印退出信息
    pub fn print_goodbye() {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                      👋 感谢使用                             ║");
        println!("║                 SCRCPY 智能启动器 v{}                  ║", env!("CARGO_PKG_VERSION"));
        println!("║                    程序已安全退出                             ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    /// 打印错误面板
    pub fn print_error_panel(title: &str, error: &str) {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║  ❌ {:54} ║", title);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  错误详情: {:47} ║", error);
        println!("║  建议: 检查网络连接和权限设置                                 ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    /// 等待用户按键
    pub fn wait_for_key() {
        print!("按任意键继续...");
        let _ = io::stdout().flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }
}