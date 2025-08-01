//! 主启动器模块
//! 整合所有功能模块，提供统一的启动器接口

use std::path::PathBuf;
use std::error::Error;
use crate::github_api::GitHubClient;
use crate::downloader::ScrcpyDownloader;
use crate::device_monitor::DeviceMonitor;

/// scrcpy智能启动器
pub struct ScrcpyLauncher {
    github_client: GitHubClient,
    downloader: ScrcpyDownloader,
    device_monitor: DeviceMonitor,
}

impl ScrcpyLauncher {
    /// 创建新的启动器实例
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let scrcpy_dir = PathBuf::from("./scrcpy");
        
        let github_client = GitHubClient::new()?;
        let downloader = ScrcpyDownloader::new(scrcpy_dir.clone());
        let device_monitor = DeviceMonitor::new(&scrcpy_dir);

        let mut launcher = Self {
            github_client,
            downloader,
            device_monitor,
        };

        // 检查scrcpy是否存在，不存在则下载
        if !launcher.is_scrcpy_available() {
            println!("🔍 未找到scrcpy");
            if launcher.ask_user_confirmation("是否从GitHub下载最新版本的scrcpy？").await {
                println!("📥 正在从GitHub下载最新版本...");
                launcher.download_latest_scrcpy().await?;
            } else {
                return Err("用户取消下载，程序无法继续运行".into());
            }
        } else {
            println!("✅ 找到scrcpy，检查是否需要更新...");
            if launcher.should_update().await? {
                if launcher.ask_user_confirmation("发现新版本，是否更新？").await {
                    println!("🔄 正在更新到最新版本...");
                    launcher.download_latest_scrcpy().await?;
                } else {
                    println!("⏭️ 跳过更新，使用当前版本");
                }
            }
        }

        // 更新设备监控器的路径（可能在下载过程中改变了）
        launcher.device_monitor = DeviceMonitor::new(&launcher.downloader.scrcpy_dir);

        Ok(launcher)
    }

    /// 检查scrcpy是否可用
    fn is_scrcpy_available(&self) -> bool {
        self.device_monitor.is_scrcpy_available()
    }

    /// 检查是否需要更新
    async fn should_update(&self) -> Result<bool, Box<dyn Error>> {
        let version_info = self.github_client.get_latest_version().await?;
        Ok(self.downloader.should_update_version(&version_info.version))
    }

    /// 下载最新的scrcpy
    async fn download_latest_scrcpy(&mut self) -> Result<(), Box<dyn Error>> {
        let version_info = self.github_client.get_latest_version().await?;
        
        self.downloader.download_scrcpy_from_url(&version_info.download_url, &version_info.version).await?;
        Ok(())
    }

    /// 询问用户确认
    async fn ask_user_confirmation(&self, message: &str) -> bool {
        use std::io::{self, Write};
        
        print!("❓ {} (y/N): ", message);
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim().to_lowercase();
                input == "y" || input == "yes" || input == "是" || input == "确定"
            }
            Err(_) => false,
        }
    }

    /// 运行启动器主循环
    pub async fn run(&mut self) {
        self.device_monitor.run().await;
    }
}
