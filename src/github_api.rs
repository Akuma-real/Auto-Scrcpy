//! GitHub API 模块
//! 处理与GitHub API的交互，获取scrcpy最新版本信息

use serde::Deserialize;
use std::error::Error;

/// GitHub发布版本信息
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<GitHubAsset>,
}

/// GitHub资源文件信息
#[derive(Debug, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// GitHub API客户端
pub struct GitHubClient {
    client: reqwest::Client,
}

impl GitHubClient {
    /// 创建新的GitHub API客户端
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 获取scrcpy的最新发布版本
    pub async fn get_latest_scrcpy_release(&self) -> Result<GitHubRelease, Box<dyn Error>> {
        let url = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest";
        
        let response = self.client
            .get(url)
            .header("User-Agent", "scrcpy-launcher")
            .send()
            .await?;
            
        let release: GitHubRelease = response.json().await?;
        Ok(release)
    }

    /// 检测系统架构
    pub fn detect_architecture() -> &'static str {
        if cfg!(target_arch = "x86_64") {
            "win64"
        } else {
            "win32"
        }
    }

    /// 查找适合当前系统的scrcpy资源
    pub fn find_suitable_asset<'a>(release: &'a GitHubRelease) -> Option<&'a GitHubAsset> {
        let arch = Self::detect_architecture();
        
        release.assets.iter()
            .find(|asset| asset.name.contains(arch) && asset.name.ends_with(".zip"))
    }
}