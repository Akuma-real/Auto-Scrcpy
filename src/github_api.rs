use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use crate::ui::TerminalUI;

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub download_url: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub enum GitHubError {
    NetworkError(reqwest::Error),
    ParseError(String),
}

impl fmt::Display for GitHubError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GitHubError::NetworkError(e) => write!(f, "网络错误: {}", e),
            GitHubError::ParseError(e) => write!(f, "解析错误: {}", e),
        }
    }
}

impl Error for GitHubError {}

impl From<reqwest::Error> for GitHubError {
    fn from(error: reqwest::Error) -> Self {
        GitHubError::NetworkError(error)
    }
}

pub struct GitHubClient {
    client: Client,
}

impl GitHubClient {
    /// 创建新的GitHub客户端
    pub fn new() -> Result<Self, GitHubError> {
        let client = Client::builder()
            .user_agent("scrcpy-launcher/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        Ok(GitHubClient { client })
    }

    /// 从仓库获取最新版本信息
    pub async fn get_latest_version(&self) -> Result<VersionInfo, GitHubError> {
        TerminalUI::print_search("正在检查最新版本信息...");
        
        // 从我们的仓库读取版本信息文件
        let url = "https://raw.githubusercontent.com/Akuma-real/Auto-Scrcpy/main/latest_version";
        
        let response = self.client
            .get(url)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(GitHubError::ParseError(format!("获取版本信息失败: {}", status)));
        }

        let version_text = response.text().await?;
        
        let version_info: VersionInfo = serde_json::from_str(&version_text)
            .map_err(|e| GitHubError::ParseError(format!("版本信息解析失败: {}", e)))?;

        TerminalUI::print_version(&format!("最新版本: {}", version_info.version));
        TerminalUI::print_info(&format!("更新时间: {}", version_info.updated_at));

        Ok(version_info)
    }

}