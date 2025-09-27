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

    /// 检查scrcpy是否可用（实时检测）
    pub fn is_scrcpy_available(&self) -> bool {
        self.scrcpy_exe.exists() && self.adb_exe.exists()
    }

    /// 检查设备连接状态（实时检测，性能优化版本）
    pub async fn check_devices(&self) -> Result<Vec<crate::tui::DeviceInfo>, String> {
        use tokio::process::Command;
        use tokio::time::{timeout, Duration};
        
        // 为 adb devices 增加命令级超时，避免 adb 异常挂死
        let output = match timeout(
            Duration::from_secs(2),
            Command::new(&self.adb_exe)
                .arg("devices")
                .output(),
        ).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(format!("执行adb命令失败: {}", e)),
            Err(_) => return Err("adb devices 命令超时".to_string()),
        };

        if !output.status.success() {
            return Err("adb devices 命令执行失败".to_string());
        }

        // 预分配容量以减少重新分配
        let mut devices = Vec::with_capacity(4); // 大多数情况下不会超过4个设备
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // 更高效的字符串处理，避免collect()
        for line in output_str.lines().skip(1) { // 跳过第一行 "List of devices attached"
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 直接使用split_once避免创建Vec
            if let Some((device_id, rest)) = line.split_once('\t') {
                if rest.trim().starts_with("device") {
                    devices.push(crate::tui::DeviceInfo {
                        id: device_id.to_string(),
                        name: "Android设备".to_string(),
                        status: "已连接".to_string(),
                    });
                }
            } else {
                // 备用解析方式（空格分隔）
                let mut parts = line.split_whitespace();
                if let (Some(device_id), Some(status)) = (parts.next(), parts.next()) {
                    if status == "device" {
                        devices.push(crate::tui::DeviceInfo {
                            id: device_id.to_string(),
                            name: "Android设备".to_string(),
                            status: "已连接".to_string(),
                        });
                    }
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

    /// 检查scrcpy进程是否还在运行
    pub fn is_scrcpy_running(&mut self) -> bool {
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
                    // 出错，假设进程已结束
                    self.scrcpy_process = None;
                    false
                }
            }
        } else {
            false
        }
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