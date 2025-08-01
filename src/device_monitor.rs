//! 设备监控模块
//! 处理Android设备连接状态监控和scrcpy进程管理

use std::process::{Command, Child, Stdio};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// 设备监控器
pub struct DeviceMonitor {
    pub adb_exe: PathBuf,
    pub scrcpy_exe: PathBuf,
    pub scrcpy_process: Option<Child>,
    pub device_connected: bool,
    pub scrcpy_window_closed: bool,
}

impl DeviceMonitor {
    /// 创建新的设备监控器
    pub fn new(scrcpy_dir: &PathBuf) -> Self {
        Self {
            adb_exe: scrcpy_dir.join("adb.exe"),
            scrcpy_exe: scrcpy_dir.join("scrcpy.exe"),
            scrcpy_process: None,
            device_connected: false,
            scrcpy_window_closed: false,
        }
    }

    /// 检查scrcpy是否可用
    pub fn is_scrcpy_available(&self) -> bool {
        self.scrcpy_exe.exists() && self.adb_exe.exists()
    }

    /// 检查设备连接状态
    pub fn check_device_connection(&self) -> bool {
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
    pub fn start_scrcpy(&mut self) -> bool {
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
                self.scrcpy_window_closed = false;
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
    pub fn is_scrcpy_running(&mut self) -> bool {
        if let Some(ref mut process) = self.scrcpy_process {
            match process.try_wait() {
                Ok(Some(_)) => {
                    // 进程已结束
                    self.scrcpy_process = None;
                    if !self.scrcpy_window_closed {
                        println!("ℹ️  scrcpy 窗口已关闭");
                        self.scrcpy_window_closed = true;
                    }
                    false
                }
                Ok(None) => {
                    // 进程仍在运行
                    true
                }
                Err(_) => {
                    // 检查失败，假设进程已结束
                    self.scrcpy_process = None;
                    if !self.scrcpy_window_closed {
                        println!("ℹ️  scrcpy 窗口已关闭");
                        self.scrcpy_window_closed = true;
                    }
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
            println!("🛑 正在关闭 scrcpy...");
            let _ = process.kill();
            let _ = process.wait();
            println!("✅ scrcpy 已关闭");
        }
    }

    /// 主监控循环
    pub async fn run(&mut self) {
        println!("🔍 scrcpy 智能启动器已启动");
        println!("📱 正在监控设备连接状态...");
        println!("💡 提示: 按 Ctrl+C 退出程序");
        println!("🔒 单实例运行: 重复启动将激活现有窗口");
        println!("----------------------------------------");

        let mut scrcpy_started = false;

        loop {
            let is_connected = self.check_device_connection();
            
            // 连接状态发生变化时打印信息
            if is_connected != self.device_connected {
                if is_connected {
                    println!("📱 检测到设备连接");
                } else {
                    println!("📱 设备已断开连接");
                }
                self.device_connected = is_connected;
            }

            if is_connected {
                // 设备已连接，启动scrcpy（如果还没启动）
                if !scrcpy_started {
                    if self.start_scrcpy() {
                        scrcpy_started = true;
                    }
                }
            } else {
                // 设备未连接，但不立即重置状态，让scrcpy进程检查来处理
            }

            // 检查scrcpy是否还在运行
            if scrcpy_started && !self.is_scrcpy_running() {
                scrcpy_started = false;
                // 只有在设备仍连接时才重新启动
                if is_connected {
                    // 等待一小段时间后重新启动
                    sleep(Duration::from_millis(500)).await;
                    if self.check_device_connection() {
                        if self.start_scrcpy() {
                            scrcpy_started = true;
                        }
                    }
                }
            }

            // 等待1秒后再次检查
            sleep(Duration::from_secs(1)).await;
        }
    }
}

impl Drop for DeviceMonitor {
    fn drop(&mut self) {
        self.stop_scrcpy();
    }
}