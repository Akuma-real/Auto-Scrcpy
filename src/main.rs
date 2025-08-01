use std::process::{Command, Child, Stdio};
use std::path::PathBuf;
use std::time::Duration;
use std::fs;
use std::io::Write;
use tokio::time::sleep;
use reqwest;
use serde::Deserialize;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

struct ScrcpyLauncher {
    scrcpy_dir: PathBuf,
    scrcpy_exe: PathBuf,
    adb_exe: PathBuf,
    scrcpy_process: Option<Child>,
}

impl ScrcpyLauncher {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scrcpy_dir = PathBuf::from("./scrcpy");
        let scrcpy_exe = scrcpy_dir.join("scrcpy.exe");
        let adb_exe = scrcpy_dir.join("adb.exe");

        let mut launcher = Self {
            scrcpy_dir,
            scrcpy_exe,
            adb_exe,
            scrcpy_process: None,
        };

        // 检查scrcpy是否存在，不存在则下载
        if !launcher.is_scrcpy_available() {
            println!("🔍 未找到scrcpy，正在从GitHub下载最新版本...");
            launcher.download_scrcpy().await?;
        } else {
            println!("✅ 找到scrcpy，检查是否需要更新...");
            if launcher.should_update().await? {
                println!("🔄 发现新版本，正在更新...");
                launcher.download_scrcpy().await?;
            }
        }

        Ok(launcher)
    }

    /// 检查scrcpy是否可用
    fn is_scrcpy_available(&self) -> bool {
        self.scrcpy_exe.exists() && self.adb_exe.exists()
    }

    /// 检查是否需要更新
    async fn should_update(&self) -> Result<bool, Box<dyn std::error::Error>> {
        // 获取本地版本
        let local_version = self.get_local_version();
        
        // 获取远程最新版本
        let latest_release = self.get_latest_release().await?;
        
        if let Some(local_ver) = local_version {
            let remote_ver = &latest_release.tag_name;
            println!("📦 本地版本: {}", local_ver);
            println!("🌐 远程版本: {}", remote_ver);
            
            // 简单的版本比较（可以改进）
            Ok(local_ver != *remote_ver)
        } else {
            Ok(true) // 没有本地版本信息，需要下载
        }
    }

    /// 获取本地版本信息
    fn get_local_version(&self) -> Option<String> {
        let version_file = self.scrcpy_dir.join("version.txt");
        fs::read_to_string(version_file).ok()
    }

    /// 保存版本信息
    fn save_version(&self, version: &str) -> Result<(), Box<dyn std::error::Error>> {
        let version_file = self.scrcpy_dir.join("version.txt");
        fs::write(version_file, version)?;
        Ok(())
    }

    /// 获取GitHub最新发布版本
    async fn get_latest_release(&self) -> Result<GitHubRelease, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest";
        
        let response = client
            .get(url)
            .header("User-Agent", "scrcpy-launcher")
            .send()
            .await?;
            
        let release: GitHubRelease = response.json().await?;
        Ok(release)
    }

    /// 检测系统架构
    fn detect_architecture() -> &'static str {
        if cfg!(target_arch = "x86_64") {
            "win64"
        } else {
            "win32"
        }
    }

    /// 下载scrcpy
    async fn download_scrcpy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let release = self.get_latest_release().await?;
        let arch = Self::detect_architecture();
        
        // 查找对应架构的资源
        let asset = release.assets.iter()
            .find(|asset| asset.name.contains(arch) && asset.name.ends_with(".zip"))
            .ok_or("未找到适合的scrcpy版本")?;

        let total_size = asset.size;
        println!("📥 正在下载: {} ({:.2} MB)", asset.name, total_size as f64 / 1024.0 / 1024.0);

        // 智能处理目录权限问题
        if self.scrcpy_dir.exists() {
            println!("🗂️  检查现有版本...");
            // 尝试清理，如果失败则使用备用目录
            if let Err(_) = fs::remove_dir_all(&self.scrcpy_dir) {
                println!("⚠️  无法清理现有目录，使用临时目录下载");
                // 使用用户临时目录
                let temp_dir = std::env::temp_dir().join("scrcpy-launcher");
                if temp_dir.exists() {
                    let _ = fs::remove_dir_all(&temp_dir);
                }
                self.scrcpy_dir = temp_dir;
                self.scrcpy_exe = self.scrcpy_dir.join("scrcpy.exe");
                self.adb_exe = self.scrcpy_dir.join("adb.exe");
            }
        }
        
        println!("📁 准备下载目录: {}", self.scrcpy_dir.display());
        if let Err(e) = fs::create_dir_all(&self.scrcpy_dir) {
            // 如果还是失败，尝试用户文档目录
            let documents_dir = dirs::document_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .join("scrcpy-launcher");
            
            println!("⚠️  使用文档目录: {}", documents_dir.display());
            self.scrcpy_dir = documents_dir;
            self.scrcpy_exe = self.scrcpy_dir.join("scrcpy.exe");
            self.adb_exe = self.scrcpy_dir.join("adb.exe");
            
            fs::create_dir_all(&self.scrcpy_dir).map_err(|e| {
                eprintln!("❌ 无法创建任何目录: {}", e);
                println!("💡 请检查磁盘空间和权限设置");
                e
            })?;
        }

        // 下载文件并显示进度
        let client = reqwest::Client::new();
        let mut response = client.get(&asset.browser_download_url).send().await?;
        
        let zip_path = self.scrcpy_dir.join(&asset.name);
        let mut file = fs::File::create(&zip_path)?;
        
        let mut downloaded = 0u64;
        let mut last_progress = 0;
        
        // 显示初始进度
        print!("📊 下载进度: [");
        for _ in 0..50 {
            print!(" ");
        }
        print!("] 0.00% (0.00 MB / {:.2} MB)\r", total_size as f64 / 1024.0 / 1024.0);
        std::io::stdout().flush()?;

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            // 计算进度百分比
            let progress = ((downloaded as f64 / total_size as f64) * 100.0) as u32;
            
            // 每增加2%或下载完成时更新进度条
            if progress != last_progress && (progress % 2 == 0 || downloaded == total_size) {
                let bar_length = (progress as f64 / 2.0) as usize; // 50个字符的进度条
                
                print!("📊 下载进度: [");
                for i in 0..50 {
                    if i < bar_length {
                        print!("█");
                    } else {
                        print!(" ");
                    }
                }
                print!("] {:.1}% ({:.2} MB / {:.2} MB)\r", 
                    progress as f64,
                    downloaded as f64 / 1024.0 / 1024.0,
                    total_size as f64 / 1024.0 / 1024.0
                );
                std::io::stdout().flush()?;
                last_progress = progress;
            }
        }
        
        // 下载完成，换行
        println!();
        println!("✅ 下载完成！");
        println!("📦 正在解压...");
        
        // 解压文件
        let file = fs::File::open(&zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => {
                    // 移除顶层目录，直接解压到scrcpy目录
                    let components: Vec<_> = path.components().collect();
                    if components.len() > 1 {
                        self.scrcpy_dir.join(components[1..].iter().collect::<PathBuf>())
                    } else {
                        continue;
                    }
                }
                None => continue,
            };

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        // 删除zip文件
        fs::remove_file(&zip_path)?;

        // 保存版本信息
        self.save_version(&release.tag_name)?;

        println!("✅ scrcpy {} 下载完成！", release.tag_name);
        Ok(())
    }

    /// 检查设备连接状态
    fn check_device_connection(&self) -> bool {
        let output = Command::new(&self.adb_exe)
            .args(&["devices"])
            .output();

        match output {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = output_str.lines().collect();
                
                // 跳过第一行 "List of devices attached"
                for line in lines.iter().skip(1) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    // 检查是否有设备且状态为 "device"
                    if line.contains("device") && !line.contains("offline") && !line.contains("unauthorized") {
                        return true;
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// 启动scrcpy
    fn start_scrcpy(&mut self) -> bool {
        if self.scrcpy_process.is_some() {
            return true; // 已经在运行
        }

        println!("🚀 启动 scrcpy...");
        
        match Command::new(&self.scrcpy_exe)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.scrcpy_process = Some(child);
                println!("✅ scrcpy 已启动");
                true
            }
            Err(e) => {
                eprintln!("❌ 启动 scrcpy 失败: {}", e);
                false
            }
        }
    }

    /// 检查scrcpy进程是否还在运行
    fn is_scrcpy_running(&mut self) -> bool {
        if let Some(ref mut process) = self.scrcpy_process {
            match process.try_wait() {
                Ok(Some(_)) => {
                    // 进程已结束
                    self.scrcpy_process = None;
                    false
                }
                Ok(None) => {
                    // 进程仍在运行
                    true
                }
                Err(_) => {
                    // 检查失败，假设进程已结束
                    self.scrcpy_process = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// 停止scrcpy
    fn stop_scrcpy(&mut self) {
        if let Some(mut process) = self.scrcpy_process.take() {
            println!("🛑 正在关闭 scrcpy...");
            let _ = process.kill();
            let _ = process.wait();
            println!("✅ scrcpy 已关闭");
        }
    }

    /// 主循环
    async fn run(&mut self) {
        println!("🔍 scrcpy 智能启动器已启动");
        println!("📱 正在监控设备连接状态...");
        println!("💡 提示: 按 Ctrl+C 退出程序");
        println!("----------------------------------------");

        let mut last_connection_status = false;
        let mut scrcpy_started = false;

        loop {
            let is_connected = self.check_device_connection();
            
            // 连接状态发生变化时打印信息
            if is_connected != last_connection_status {
                if is_connected {
                    println!("📱 检测到设备连接");
                } else {
                    println!("📱 设备已断开连接");
                }
                last_connection_status = is_connected;
            }

            if is_connected {
                // 设备已连接，启动scrcpy（如果还没启动）
                if !scrcpy_started {
                    if self.start_scrcpy() {
                        scrcpy_started = true;
                    }
                }
            } else {
                // 设备未连接，重置状态
                if scrcpy_started {
                    scrcpy_started = false;
                }
            }

            // 检查scrcpy是否还在运行
            if scrcpy_started && !self.is_scrcpy_running() {
                println!("ℹ️  scrcpy 窗口已关闭");
                scrcpy_started = false;
            }

            // 等待1秒后再次检查
            sleep(Duration::from_secs(1)).await;
        }
    }
}

impl Drop for ScrcpyLauncher {
    fn drop(&mut self) {
        self.stop_scrcpy();
    }
}

#[tokio::main]
async fn main() {
    println!("🚀 scrcpy 智能启动器 v0.1.0");
    println!("🌐 支持自动下载最新版本");
    println!("========================================");

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
            launcher.stop_scrcpy();
            println!("👋 再见！");
        }
    }
}