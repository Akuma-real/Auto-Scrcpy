//! scrcpy 智能启动器
//! 自动检测设备连接并启动scrcpy

mod single_instance;
mod device_monitor;
mod tui;

use single_instance::SingleInstanceGuard;
use tui::{TuiApp, LogLevel, DeviceInfo};
use device_monitor::DeviceMonitor;

use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    // 单实例检查
    let _guard = match SingleInstanceGuard::new("scrcpy-launcher") {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("❌ 单实例检查失败: {}", e);
            return;
        }
    };

    // 创建TUI应用
    let mut app = match TuiApp::new() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("❌ TUI初始化失败: {}", e);
            return;
        }
    };

    // 添加初始化日志
    app.state_mut().add_log(LogLevel::Success, "单实例检查通过".to_string());
    app.state_mut().add_log(LogLevel::Info, "SCRCPY 智能启动器已启动".to_string());

    // 创建共享状态
    let app_state = Arc::new(Mutex::new(app.state().clone()));

    // 创建消息通道
    let (tx, mut rx) = mpsc::channel(100);

    // 启动业务逻辑任务
    let business_handle = tokio::spawn(async move {
        run_device_monitor(tx).await;
    });

    // 启动TUI更新任务
    let app_state_for_tui = app_state.clone();
    let tui_handle = tokio::spawn(async move {
        // 处理来自业务逻辑的消息
        while let Some(msg) = rx.recv().await {
            let mut state = app_state_for_tui.lock().await;
            match msg {
                TuiMessage::Log(level, message) => {
                    state.add_log(level, message);
                }
                TuiMessage::Status(status) => {
                    state.set_status(status);
                }
                TuiMessage::UpdateDevices(devices) => {
                    state.update_devices(devices);
                }
                TuiMessage::Quit => {
                    state.should_quit = true;
                    break;
                }
            }
        }
    });

    // 运行TUI主循环
    let result = tokio::select! {
        result = app.run_with_shared_state(app_state) => result,
        _ = tokio::signal::ctrl_c() => {
            Ok(())
        }
    };

    // 清理
    business_handle.abort();
    tui_handle.abort();

    if let Err(e) = result {
        eprintln!("❌ 程序运行错误: {}", e);
    }
}

/// TUI消息类型
#[derive(Debug)]
pub enum TuiMessage {
    Log(LogLevel, String),
    Status(String),
    UpdateDevices(Vec<DeviceInfo>),
    Quit,
}

/// 运行设备监控逻辑（性能优化版本）
async fn run_device_monitor(tx: mpsc::Sender<TuiMessage>) {
    let _ = tx.send(TuiMessage::Status("监控设备连接...".to_string())).await;
    let _ = tx.send(TuiMessage::Log(LogLevel::Info, "开始监控Android设备连接".to_string())).await;

    // 获取scrcpy目录
    let scrcpy_dir = get_scrcpy_directory();
    let mut device_monitor = DeviceMonitor::new(&scrcpy_dir);
    let mut scrcpy_started = false;
    let mut last_device_id: Option<String> = None;
    let mut last_status_update = std::time::Instant::now();
    let mut last_device_count = 0;
    let mut consecutive_checks = 0;
    
    // 预分配字符串以减少内存分配
    let status_waiting = "等待设备连接中...".to_string();

    loop {
        consecutive_checks += 1;
        
        // 并行执行设备检查和状态更新
        let device_check_result = tokio::select! {
            result = check_connected_devices_with_monitor(&device_monitor) => result,
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                // 50ms超时，如果adb命令太慢就跳过这次检查
                continue;
            }
        };
        
        if let Ok(devices) = device_check_result {
            // 只在设备列表实际变化时更新UI
            let device_count = devices.len();
            let device_count_changed = device_count != last_device_count;
            
            if device_count_changed || consecutive_checks % 10 == 0 {
                // 每10次检查或设备变化时更新UI
                let _ = tx.send(TuiMessage::UpdateDevices(devices.clone())).await;
            }
            
            last_device_count = device_count;
            
            if !devices.is_empty() {
                let current_device_id = &devices[0].id; // 使用引用避免clone
                
                // 检查scrcpy进程状态（如果认为已启动）
                if scrcpy_started {
                    if !device_monitor.is_scrcpy_running() {
                        let _ = tx.send(TuiMessage::Log(
                            LogLevel::Warning,
                            "检测到scrcpy进程已结束，正在自动重启...".to_string()
                        )).await;
                        scrcpy_started = false; // 重置状态以触发重启
                    }
                }
                
                // 在设备变化、scrcpy未启动或设备数量变化时启动
                if !scrcpy_started || last_device_id.as_ref() != Some(current_device_id) || device_count_changed {
                    // 只在设备真正变化时显示发现日志
                    if last_device_id.as_ref() != Some(current_device_id) || device_count_changed {
                        for device in &devices {
                            let _ = tx.send(TuiMessage::Log(
                                LogLevel::Device,
                                format!("发现设备: {} ({})", device.name, device.id)
                            )).await;
                        }
                    }
                    
                    let _ = tx.send(TuiMessage::Log(LogLevel::Launch, "正在启动scrcpy...".to_string())).await;
                    
                    if device_monitor.is_scrcpy_available() {
                        match device_monitor.start_scrcpy(Some(current_device_id)) {
                            Ok(_) => {
                                let _ = tx.send(TuiMessage::Log(
                                    LogLevel::Success,
                                    format!("成功启动scrcpy连接设备: {}", devices[0].name)
                                )).await;
                                scrcpy_started = true;
                                last_device_id = Some(current_device_id.clone());
                            }
                            Err(e) => {
                                let _ = tx.send(TuiMessage::Log(
                                    LogLevel::Error,
                                    format!("启动scrcpy失败: {}", e)
                                )).await;
                                scrcpy_started = false;
                            }
                        }
                    } else {
                        let _ = tx.send(TuiMessage::Log(
                            LogLevel::Error,
                            "scrcpy或adb未找到，请确保scrcpy已正确安装".to_string()
                        )).await;
                    }
                }
            } else {
                // 没有设备连接时，重置状态
                if scrcpy_started {
                    if let Some(device_id) = &last_device_id {
                        let _ = tx.send(TuiMessage::Log(
                            LogLevel::Warning,
                            format!("设备已断开连接: {}", device_id)
                        )).await;
                    }
                    device_monitor.stop_scrcpy();
                    scrcpy_started = false;
                    last_device_id = None;
                }
                
                // 减少状态提示频率，从30秒增加到60秒
                if last_status_update.elapsed().as_secs() >= 60 {
                    let _ = tx.send(TuiMessage::Log(LogLevel::Info, status_waiting.clone())).await;
                    last_status_update = std::time::Instant::now();
                }
            }
        }
        
        // 动态调整检查间隔：更激进的优化策略
        let check_interval = if consecutive_checks < 50 {
            // 前12.5秒每100ms检查一次（超快响应初始连接）
            Duration::from_millis(100)
        } else if scrcpy_started && last_device_count > 0 {
            // 设备已连接且scrcpy运行时，适度降低频率
            Duration::from_millis(250)
        } else {
            // 等待设备连接时保持高频率
            Duration::from_millis(150)
        };
        
        sleep(check_interval).await;
    }
}

/// 检查连接的设备（使用传入的设备监控器实例）
async fn check_connected_devices_with_monitor(device_monitor: &DeviceMonitor) -> Result<Vec<DeviceInfo>, String> {
    // 检查adb是否可用
    if !device_monitor.adb_exe.exists() {
        return Err("ADB未找到，请确保scrcpy已正确安装".to_string());
    }
    
    // 使用设备监控器检查设备
    device_monitor.check_devices().await
}


/// 获取scrcpy目录
fn get_scrcpy_directory() -> PathBuf {
    // 首先尝试当前目录下的scrcpy文件夹
    let current_dir_scrcpy = std::env::current_dir()
        .unwrap_or_default()
        .join("scrcpy");
    
    if current_dir_scrcpy.exists() {
        return current_dir_scrcpy;
    }
    
    // 然后尝试用户目录下的scrcpy文件夹
    if let Some(home_dir) = dirs::home_dir() {
        let home_scrcpy = home_dir.join("scrcpy");
        if home_scrcpy.exists() {
            return home_scrcpy;
        }
    }
    
    // 最后尝试程序文件目录
    let program_files = PathBuf::from("C:\\Program Files\\scrcpy");
    if program_files.exists() {
        return program_files;
    }
    
    // 默认返回当前目录下的scrcpy文件夹
    current_dir_scrcpy
}