//! 单实例保护模块
//! 确保程序只能运行一个实例

#[cfg(target_os = "windows")]
use winapi::um::synchapi::CreateMutexA;
#[cfg(target_os = "windows")]
use winapi::um::handleapi::CloseHandle;
#[cfg(target_os = "windows")]
use winapi::um::errhandlingapi::GetLastError;
#[cfg(target_os = "windows")]
use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
#[cfg(target_os = "windows")]
use winapi::um::winbase::OpenMutexA;
#[cfg(target_os = "windows")]
use winapi::um::winnt::SYNCHRONIZE;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{SetForegroundWindow, ShowWindow, IsIconic};
#[cfg(target_os = "windows")]
use winapi::um::wincon::GetConsoleWindow;
#[cfg(target_os = "windows")]
use std::ffi::CString;
#[cfg(target_os = "windows")]
use std::ptr;

/// 单实例保护结构
#[cfg(target_os = "windows")]
pub struct SingleInstanceGuard {
    mutex_handle: winapi::um::winnt::HANDLE,
}

#[cfg(target_os = "windows")]
impl SingleInstanceGuard {
    /// 创建单实例保护
    /// 如果已有实例在运行，会尝试激活现有窗口并退出程序
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        unsafe {
            // 使用包名而不是版本号，确保不同版本之间也能互斥
            let mutex_name = CString::new("Global\\ScrcpyLauncher_SingleInstance")?;
            
            // 首先尝试打开现有的互斥锁
            let existing_mutex = OpenMutexA(SYNCHRONIZE, 0, mutex_name.as_ptr());
            if !existing_mutex.is_null() {
                // 找到现有实例，尝试激活它
                CloseHandle(existing_mutex);
                println!("⚠️  检测到程序已在运行");
                Self::find_and_activate_existing_window();
                println!("🔄 已激活现有程序窗口");
                std::process::exit(0);
            }
            
            // 没有现有实例，创建新的互斥锁
            let mutex_handle = CreateMutexA(ptr::null_mut(), 1, mutex_name.as_ptr());
            if mutex_handle.is_null() {
                return Err("无法创建互斥锁".into());
            }
            
            // 检查是否因为已存在而失败
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(mutex_handle);
                println!("⚠️  检测到程序已在运行");
                Self::find_and_activate_existing_window();
                println!("🔄 已激活现有程序窗口");
                std::process::exit(0);
            }
            
            Ok(SingleInstanceGuard { mutex_handle })
        }
    }
    
    /// 查找并激活现有的程序窗口
    fn find_and_activate_existing_window() {
        unsafe {
            let console_window = GetConsoleWindow();
            if !console_window.is_null() {
                if IsIconic(console_window) != 0 {
                    ShowWindow(console_window, 9); // SW_RESTORE = 9
                }
                SetForegroundWindow(console_window);
                return;
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.mutex_handle);
        }
    }
}

/// 非Windows平台的单实例保护（简化版）
#[cfg(not(target_os = "windows"))]
pub struct SingleInstanceGuard;

#[cfg(not(target_os = "windows"))]
impl SingleInstanceGuard {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let lock_file = std::env::temp_dir().join("scrcpy_launcher.lock");
        if lock_file.exists() {
            println!("⚠️  检测到程序可能已在运行");
            println!("💡 如果确认没有其他实例，请删除锁文件: {}", lock_file.display());
        } else {
            std::fs::write(&lock_file, std::process::id().to_string())?;
        }
        Ok(SingleInstanceGuard)
    }
}

#[cfg(not(target_os = "windows"))]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let lock_file = std::env::temp_dir().join("scrcpy_launcher.lock");
        let _ = std::fs::remove_file(lock_file);
    }
}