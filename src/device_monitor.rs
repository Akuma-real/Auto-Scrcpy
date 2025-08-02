//! 设备监控模块
//! 处理Android设备连接状态监控和scrcpy进程管理

use std::process::Child;
use std::path::PathBuf;

/// 设备监控器
pub struct DeviceMonitor {
    pub adb_exe: PathBuf,
    pub scrcpy_exe: PathBuf,
    pub scrcpy_process: Option<Child>,
}

impl DeviceMonitor {
    /// 创建新的设备监控器
    pub fn new(scrcpy_dir: &PathBuf) -> Self {
        Self {
            adb_exe: scrcpy_dir.join("adb.exe"),
            scrcpy_exe: scrcpy_dir.join("scrcpy.exe"),
            scrcpy_process: None,
        }
    }

    /// 检查scrcpy是否可用
    pub fn is_scrcpy_available(&self) -> bool {
        self.scrcpy_exe.exists() && self.adb_exe.exists()
    }

    /// 检查设备连接状态
    pub async fn check_devices(&self) -> Result<Vec<crate::tui::DeviceInfo>, String> {
        use tokio::process::Command;
        
        let output = Command::new(&self.adb_exe)
            .arg("devices")
            .output()
            .await
            .map_err(|e| format!("执行adb命令失败: {}", e))?;

        if !output.status.success() {
            return Err("adb devices 命令执行失败".to_string());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in output_str.lines().skip(1) { // 跳过第一行 "List of devices attached"
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device_id = parts[0].to_string();
                let status = parts[1].to_string();
                
                // 只添加已连接的设备
                if status == "device" {
                    devices.push(crate::tui::DeviceInfo {
                        id: device_id.clone(),
                        name: format!("Android设备 {}", &device_id[..std::cmp::min(8, device_id.len())]),
                        status: "已连接".to_string(),
                    });
                }
            }
        }

        Ok(devices)
    }

    /// 启动scrcpy（重定向输出以避免干扰TUI）
    pub fn start_scrcpy(&mut self, device_id: Option<&str>) -> Result<(), String> {
        use std::process::{Command, Stdio};

        // 停止现有的scrcpy进程
        self.stop_scrcpy();

        let mut cmd = Command::new(&self.scrcpy_exe);
        
        if let Some(id) = device_id {
            cmd.arg("-s").arg(id);
        }

        // 重定向输出以避免干扰TUI界面
        cmd.stdout(Stdio::null())
           .stderr(Stdio::null())
           .stdin(Stdio::null());

        let child = cmd.spawn()
            .map_err(|e| format!("启动scrcpy失败: {}", e))?;

        self.scrcpy_process = Some(child);
        Ok(())
    }

    /// 停止scrcpy
    pub fn stop_scrcpy(&mut self) {
        if let Some(mut process) = self.scrcpy_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Drop for DeviceMonitor {
    fn drop(&mut self) {
        self.stop_scrcpy();
    }
}