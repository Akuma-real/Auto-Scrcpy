//! 下载器模块
//! 处理scrcpy的下载和解压功能

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::error::Error;
use zip::ZipArchive;

/// scrcpy下载器
pub struct ScrcpyDownloader {
    pub scrcpy_dir: PathBuf,
    client: reqwest::Client,
}

impl ScrcpyDownloader {
    /// 创建新的下载器
    pub fn new(scrcpy_dir: PathBuf) -> Self {
        Self {
            scrcpy_dir,
            client: reqwest::Client::new(),
        }
    }

    /// 检查本地版本
    pub fn get_local_version(&self) -> Option<String> {
        let version_file = self.scrcpy_dir.join("version.txt");
        fs::read_to_string(version_file).ok()
    }

    /// 保存版本信息
    pub fn save_version(&self, version: &str) -> Result<(), Box<dyn Error>> {
        let version_file = self.scrcpy_dir.join("version.txt");
        fs::write(version_file, version)?;
        Ok(())
    }

    /// 检查版本是否需要更新
    pub fn should_update_version(&self, remote_version: &str) -> bool {
        if let Some(local_ver) = self.get_local_version() {
            println!("📦 本地版本: {}", local_ver);
            println!("🌐 远程版本: {}", remote_version);
            
            local_ver != remote_version
        } else {
            true // 没有本地版本信息，需要下载
        }
    }

    /// 准备下载目录
    pub fn prepare_download_directory(&mut self) -> Result<(), Box<dyn Error>> {
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
            }
        }
        
        println!("📁 准备下载目录: {}", self.scrcpy_dir.display());
        if let Err(_e) = fs::create_dir_all(&self.scrcpy_dir) {
            // 如果还是失败，尝试用户文档目录
            let documents_dir = dirs::document_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .join("scrcpy-launcher");
            
            println!("⚠️  使用文档目录: {}", documents_dir.display());
            self.scrcpy_dir = documents_dir;
            
            fs::create_dir_all(&self.scrcpy_dir).map_err(|e| {
                eprintln!("❌ 无法创建任何目录: {}", e);
                println!("💡 请检查磁盘空间和权限设置");
                e
            })?;
        }

        Ok(())
    }

    /// 从URL直接下载scrcpy
    pub async fn download_scrcpy_from_url(&mut self, download_url: &str, version: &str) -> Result<(), Box<dyn Error>> {
        println!("📥 正在下载scrcpy {}...", version);

        // 准备下载目录
        self.prepare_download_directory()?;

        // 获取文件名
        let filename = download_url.split('/').last().unwrap_or("scrcpy.zip");
        println!("📦 文件名: {}", filename);

        // 下载文件
        let mut response = self.client.get(download_url).send().await?;
        
        let zip_path = self.scrcpy_dir.join(filename);
        let mut file = fs::File::create(&zip_path)?;
        
        let mut downloaded = 0u64;
        let total_size = response.content_length().unwrap_or(0);
        
        if total_size > 0 {
            println!("📊 文件大小: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
            self.print_progress(0, 0.0, total_size as f64 / 1024.0 / 1024.0)?;
        }

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let progress = ((downloaded as f64 / total_size as f64) * 100.0) as u32;
                if progress % 5 == 0 { // 每5%更新一次
                    self.print_progress(progress, downloaded as f64 / 1024.0 / 1024.0, total_size as f64 / 1024.0 / 1024.0)?;
                }
            }
        }
        
        println!();
        println!("✅ 下载完成！");
        println!("📦 正在解压...");
        
        // 解压文件
        self.extract_zip(&zip_path)?;

        // 删除zip文件
        fs::remove_file(&zip_path)?;

        // 保存版本信息
        self.save_version(version)?;

        println!("✅ scrcpy {} 安装完成！", version);
        Ok(())
    }

    /// 打印下载进度
    fn print_progress(&self, progress: u32, downloaded_mb: f64, total_mb: f64) -> Result<(), Box<dyn Error>> {
        let bar_length = (progress as f64 / 2.0) as usize; // 50个字符的进度条
        
        print!("📊 下载进度: [");
        for i in 0..50 {
            if i < bar_length {
                print!("█");
            } else {
                print!(" ");
            }
        }
        print!("] {:.1}% ({:.2} MB / {:.2} MB)\r", progress as f64, downloaded_mb, total_mb);
        std::io::stdout().flush()?;
        Ok(())
    }

    /// 解压ZIP文件
    fn extract_zip(&self, zip_path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let file = fs::File::open(zip_path)?;
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

        Ok(())
    }
}