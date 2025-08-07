//! 自动更新模块
//! 处理版本检查、下载和自动更新功能

use std::path::PathBuf;
use std::fs;
use std::io::Write;
use serde::{Deserialize, Serialize};
use reqwest;
use tempfile::NamedTempFile;
use tokio::sync::mpsc;
use crate::tui::{LogLevel, DownloadProgress, VersionInfo};
use crate::TuiMessage;

/// 远程版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteVersionInfo {
    pub version: String,
    pub download_url: String,
    pub updated_at: String,
}

/// 版本文件格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub launcher_version: String,
    pub scrcpy_version: String,
    pub scrcpy_download_url: String,
    pub updated_at: String,
    pub changelog: VersionChangelog,
}

/// 版本变更日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChangelog {
    pub launcher_changed: bool,
    pub scrcpy_changed: bool,
}

/// 更新器配置
#[derive(Debug, Clone)]
pub struct UpdaterConfig {
    pub check_url: String,
    pub auto_check: bool,
    pub auto_download: bool,
    pub check_interval_hours: u64,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            check_url: "https://api.github.com/repos/your-username/Auto-Scrcpy/releases/latest".to_string(),
            auto_check: true,
            auto_download: false,
            check_interval_hours: 24,
        }
    }
}

/// 更新器
pub struct Updater {
    config: UpdaterConfig,
    current_version: String,
    client: reqwest::Client,
}

impl Updater {
    /// 创建新的更新器
    pub fn new(config: UpdaterConfig) -> Self {
        Self {
            config,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// 检查是否有新版本
    pub async fn check_for_updates(&self) -> Result<Option<RemoteVersionInfo>, String> {
        let response = self.client
            .get(&self.config.check_url)
            .header("User-Agent", "scrcpy-launcher")
            .send()
            .await
            .map_err(|e| format!("网络请求失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("服务器返回错误: {}", response.status()));
        }

        let release_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析JSON失败: {}", e))?;

        // 从GitHub API响应中提取版本信息
        let version = release_data["tag_name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 查找Windows下载链接（查找scrcpy-launcher的可执行文件）
        let empty_vec = vec![];
        let assets = release_data["assets"].as_array().unwrap_or(&empty_vec);
        let download_url = assets
            .iter()
            .find(|asset| {
                let name = asset["name"].as_str().unwrap_or("");
                (name.contains("win64") || name.contains("windows"))
                    && (name.ends_with(".exe") || name.ends_with(".zip"))
                    && name.contains("scrcpy-launcher")
            })
            .and_then(|asset| asset["browser_download_url"].as_str())
            .unwrap_or("")
            .to_string();

        if version.is_empty() || download_url.is_empty() {
            return Err("无法找到有效的版本信息".to_string());
        }

        let remote_info = RemoteVersionInfo {
            version: version.clone(),
            download_url,
            updated_at: release_data["published_at"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        };

        // 比较版本
        if self.is_newer_version(&version)? {
            Ok(Some(remote_info))
        } else {
            Ok(None)
        }
    }

    /// 比较版本号
    fn is_newer_version(&self, remote_version: &str) -> Result<bool, String> {
        // 移除可能的 'v' 前缀
        let current = self.current_version.trim_start_matches('v');
        let remote = remote_version.trim_start_matches('v');

        // 使用 semver 进行版本比较
        let current_ver = semver::Version::parse(current)
            .map_err(|e| format!("解析当前版本失败: {}", e))?;
        let remote_ver = semver::Version::parse(remote)
            .map_err(|e| format!("解析远程版本失败: {}", e))?;

        Ok(remote_ver > current_ver)
    }

    /// 下载更新文件
    pub async fn download_update(
        &self,
        remote_info: &RemoteVersionInfo,
        tx: &mpsc::Sender<TuiMessage>,
    ) -> Result<PathBuf, String> {
        let _ = tx.send(TuiMessage::Log(
            LogLevel::Download,
            format!("开始下载版本 {}", remote_info.version)
        )).await;

        // 创建临时文件
        let temp_file = NamedTempFile::new()
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        let temp_path = temp_file.path().to_path_buf();

        // 开始下载
        let response = self.client
            .get(&remote_info.download_url)
            .send()
            .await
            .map_err(|e| format!("下载请求失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("下载失败: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();
        let mut file = fs::File::create(&temp_path)
            .map_err(|e| format!("创建文件失败: {}", e))?;

        let start_time = std::time::Instant::now();

        // 使用流式下载并显示进度
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("下载数据失败: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;

            downloaded += chunk.len() as u64;

            // 计算并发送进度
            let progress = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64 / 1024.0 / 1024.0) / elapsed
            } else {
                0.0
            };

            let download_progress = DownloadProgress {
                filename: format!("scrcpy-{}", remote_info.version),
                progress,
                downloaded_mb: downloaded as f64 / 1024.0 / 1024.0,
                total_mb: total_size as f64 / 1024.0 / 1024.0,
                speed_mbps: speed,
            };

            let _ = tx.send(TuiMessage::UpdateDownloadProgress(download_progress)).await;
        }

        file.sync_all()
            .map_err(|e| format!("同步文件失败: {}", e))?;

        let _ = tx.send(TuiMessage::Log(
            LogLevel::Success,
            "下载完成".to_string()
        )).await;

        // 防止临时文件被删除
        let final_path = temp_file.into_temp_path().keep()
            .map_err(|e| format!("保存临时文件失败: {}", e.error))?;

        Ok(final_path)
    }

    /// 执行自动更新
    pub async fn perform_update(
        &self,
        download_path: &PathBuf,
        tx: &mpsc::Sender<TuiMessage>,
    ) -> Result<(), String> {
        let _ = tx.send(TuiMessage::Log(
            LogLevel::Info,
            "准备安装更新...".to_string()
        )).await;

        // 获取当前可执行文件路径
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("获取当前程序路径失败: {}", e))?;

        let current_dir = current_exe.parent()
            .ok_or("无法获取程序目录")?;

        // 解压下载的zip文件
        let extract_dir = current_dir.join("temp_update");
        self.extract_zip(download_path, &extract_dir, tx).await?;

        // 查找本程序的可执行文件
        let new_exe = self.find_program_exe(&extract_dir)?;
        
        // 创建备份
        let backup_path = current_dir.join("scrcpy_launcher_backup");
        self.create_program_backup(&current_exe, &backup_path, tx).await?;

        // 复制新的可执行文件
        self.copy_new_executable(&new_exe, &current_exe, tx).await?;

        // 清理临时文件
        let _ = fs::remove_dir_all(&extract_dir);
        let _ = fs::remove_file(download_path);

        let _ = tx.send(TuiMessage::Log(
            LogLevel::Success,
            "更新安装完成！程序将重启以应用更新。".to_string()
        )).await;

        Ok(())
    }

    /// 解压ZIP文件
    async fn extract_zip(
        &self,
        zip_path: &PathBuf,
        extract_to: &PathBuf,
        tx: &mpsc::Sender<TuiMessage>,
    ) -> Result<(), String> {
        let _ = tx.send(TuiMessage::Log(
            LogLevel::Info,
            "正在解压更新文件...".to_string()
        )).await;

        fs::create_dir_all(extract_to)
            .map_err(|e| format!("创建解压目录失败: {}", e))?;

        let file = fs::File::open(zip_path)
            .map_err(|e| format!("打开zip文件失败: {}", e))?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("读取zip文件失败: {}", e))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| format!("读取zip条目失败: {}", e))?;

            let outpath = extract_to.join(file.name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)
                    .map_err(|e| format!("创建目录失败: {}", e))?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)
                        .map_err(|e| format!("创建父目录失败: {}", e))?;
                }

                let mut outfile = fs::File::create(&outpath)
                    .map_err(|e| format!("创建文件失败: {}", e))?;

                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("复制文件失败: {}", e))?;
            }
        }

        Ok(())
    }

    /// 查找本程序的可执行文件
    fn find_program_exe(&self, extract_dir: &PathBuf) -> Result<PathBuf, String> {
        // 递归查找scrcpy-launcher.exe
        fn find_exe_recursive(dir: &PathBuf) -> Result<PathBuf, String> {
            for entry in fs::read_dir(dir).map_err(|e| format!("读取目录失败: {}", e))? {
                let entry = entry.map_err(|e| format!("读取条目失败: {}", e))?;
                let path = entry.path();

                if path.is_file() {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    if filename.contains("scrcpy-launcher") && filename.ends_with(".exe") {
                        return Ok(path);
                    }
                } else if path.is_dir() {
                    if let Ok(found) = find_exe_recursive(&path) {
                        return Ok(found);
                    }
                }
            }
            Err("未找到scrcpy-launcher.exe".to_string())
        }

        find_exe_recursive(extract_dir)
    }


    /// 创建程序备份
    async fn create_program_backup(
        &self,
        current_exe: &PathBuf,
        backup_path: &PathBuf,
        tx: &mpsc::Sender<TuiMessage>,
    ) -> Result<(), String> {
        let _ = tx.send(TuiMessage::Log(
            LogLevel::Info,
            "创建程序备份...".to_string()
        )).await;

        if backup_path.exists() {
            fs::remove_file(backup_path)
                .map_err(|e| format!("删除旧备份失败: {}", e))?;
        }

        fs::copy(current_exe, backup_path)
            .map_err(|e| format!("创建程序备份失败: {}", e))?;

        Ok(())
    }

    /// 复制新的可执行文件
    async fn copy_new_executable(
        &self,
        new_exe: &PathBuf,
        current_exe: &PathBuf,
        tx: &mpsc::Sender<TuiMessage>,
    ) -> Result<(), String> {
        let _ = tx.send(TuiMessage::Log(
            LogLevel::Info,
            "更新程序文件...".to_string()
        )).await;

        // 创建临时文件名
        let temp_exe = current_exe.with_extension("exe.new");
        
        // 先复制到临时文件
        fs::copy(new_exe, &temp_exe)
            .map_err(|e| format!("复制新程序失败: {}", e))?;

        // 如果是Windows，需要特殊处理正在运行的可执行文件
        #[cfg(windows)]
        {
            // 在Windows上，重命名当前运行的exe为.old
            let old_exe = current_exe.with_extension("exe.old");
            if old_exe.exists() {
                let _ = fs::remove_file(&old_exe);
            }
            
            // 将当前exe重命名为.old
            fs::rename(current_exe, &old_exe)
                .map_err(|e| format!("重命名当前程序失败: {}", e))?;

            // 将新exe重命名为当前程序名
            fs::rename(&temp_exe, current_exe)
                .map_err(|e| format!("安装新程序失败: {}", e))?;
        }

        #[cfg(not(windows))]
        {
            // 在非Windows系统上直接替换
            fs::rename(&temp_exe, current_exe)
                .map_err(|e| format!("安装新程序失败: {}", e))?;
        }

        Ok(())
    }

    /// 检查并执行自动更新流程
    pub async fn auto_update_check(&self, tx: mpsc::Sender<TuiMessage>) -> Result<(), String> {
        if !self.config.auto_check {
            return Ok(());
        }

        let _ = tx.send(TuiMessage::Log(
            LogLevel::Info,
            "检查更新中...".to_string()
        )).await;

        match self.check_for_updates().await {
            Ok(Some(remote_info)) => {
                let version_info = VersionInfo {
                    local: self.current_version.clone(),
                    remote: remote_info.version.clone(),
                    needs_update: true,
                };

                let _ = tx.send(TuiMessage::UpdateVersionInfo(version_info)).await;
                let _ = tx.send(TuiMessage::Log(
                    LogLevel::Info,
                    format!("发现新版本: {} -> {}", self.current_version, remote_info.version)
                )).await;

                if self.config.auto_download {
                    let _ = tx.send(TuiMessage::Log(
                        LogLevel::Info,
                        "开始自动下载更新...".to_string()
                    )).await;

                    match self.download_update(&remote_info, &tx).await {
                        Ok(download_path) => {
                            let _ = tx.send(TuiMessage::Log(
                                LogLevel::Success,
                                "下载完成，准备安装...".to_string()
                            )).await;

                            if let Err(e) = self.perform_update(&download_path, &tx).await {
                                let _ = tx.send(TuiMessage::Log(
                                    LogLevel::Error,
                                    format!("更新安装失败: {}", e)
                                )).await;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(TuiMessage::Log(
                                LogLevel::Error,
                                format!("下载失败: {}", e)
                            )).await;
                        }
                    }
                }
            }
            Ok(None) => {
                let _ = tx.send(TuiMessage::Log(
                    LogLevel::Info,
                    "当前已是最新版本".to_string()
                )).await;
            }
            Err(e) => {
                let _ = tx.send(TuiMessage::Log(
                    LogLevel::Warning,
                    format!("检查更新失败: {}", e)
                )).await;
            }
        }

        Ok(())
    }
}