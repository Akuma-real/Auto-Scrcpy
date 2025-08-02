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
use winapi::um::winuser::{
    SetForegroundWindow, ShowWindow, IsIconic, FindWindowA, EnumWindows, 
    GetWindowTextA, GetWindowThreadProcessId, AttachThreadInput, 
    BringWindowToTop, SetActiveWindow
};
#[cfg(target_os = "windows")]
use winapi::um::wincon::GetConsoleWindow;
#[cfg(target_os = "windows")]
use winapi::um::processthreadsapi::{GetCurrentProcessId, GetCurrentThreadId};
#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
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
            // 方法1: 尝试激活控制台窗口
            let console_window = GetConsoleWindow();
            if !console_window.is_null() {
                Self::activate_window(console_window);
                return;
            }
            
            // 方法2: 通过窗口标题查找
            let window_title = CString::new("scrcpy 智能启动器").unwrap_or_default();
            let window_handle = FindWindowA(ptr::null(), window_title.as_ptr());
            if !window_handle.is_null() {
                Self::activate_window(window_handle);
                return;
            }
            
            // 方法3: 枚举所有窗口查找匹配的进程
            EnumWindows(Some(Self::enum_windows_proc), 0);
        }
    }
    
    /// 激活指定窗口
    fn activate_window(window_handle: HWND) {
        unsafe {
            // 如果窗口被最小化，先还原
            if IsIconic(window_handle) != 0 {
                ShowWindow(window_handle, 9); // SW_RESTORE = 9
            }
            
            // 获取窗口线程ID
            let mut process_id = 0;
            let window_thread_id = GetWindowThreadProcessId(window_handle, &mut process_id);
            let current_thread_id = GetCurrentThreadId();
            
            // 如果是不同线程，需要附加输入
            if window_thread_id != current_thread_id {
                AttachThreadInput(current_thread_id, window_thread_id, 1);
            }
            
            // 激活窗口
            BringWindowToTop(window_handle);
            SetActiveWindow(window_handle);
            SetForegroundWindow(window_handle);
            
            // 分离输入
            if window_thread_id != current_thread_id {
                AttachThreadInput(current_thread_id, window_thread_id, 0);
            }
            
            // 确保窗口可见
            ShowWindow(window_handle, 5); // SW_SHOW = 5
        }
    }
    
    /// 枚举窗口的回调函数
    unsafe extern "system" fn enum_windows_proc(
        window_handle: HWND,
        _lparam: winapi::shared::minwindef::LPARAM,
    ) -> winapi::shared::minwindef::BOOL {
        let mut process_id = 0;
        GetWindowThreadProcessId(window_handle, &mut process_id);
        
        // 检查是否是当前进程的窗口
        if process_id == GetCurrentProcessId() {
            // 获取窗口标题
            let mut title_buffer = [0i8; 256];
            let title_len = GetWindowTextA(window_handle, title_buffer.as_mut_ptr(), 256);
            
            if title_len > 0 {
                let title = std::ffi::CStr::from_ptr(title_buffer.as_ptr())
                    .to_string_lossy();
                
                // 如果窗口标题包含程序相关关键词，激活它
                if title.contains("scrcpy") || title.contains("启动器") {
                    Self::activate_window(window_handle);
                    return 0; // 停止枚举
                }
            }
        }
        
        1 // 继续枚举
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