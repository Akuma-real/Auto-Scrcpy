use std::ffi::CString;
use std::ptr;
use winapi::um::winuser::{
    FindWindowA, GetWindowThreadProcessId, IsIconic, ShowWindow, SetForegroundWindow,
    BringWindowToTop, SetActiveWindow, AttachThreadInput,
    SW_RESTORE, SW_SHOW, EnumWindows, GetWindowTextA
};
use winapi::um::processthreadsapi::{GetCurrentProcessId, GetCurrentThreadId};
use winapi::shared::windef::HWND;
use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE, FALSE};

pub struct SingleInstanceGuard {
    _mutex_name: String,
}

// 全局变量用于窗口查找
static mut TARGET_PROCESS_ID: DWORD = 0;
static mut FOUND_WINDOW: HWND = ptr::null_mut();

impl SingleInstanceGuard {
    pub fn new(app_name: &str) -> Result<Self, String> {
        let mutex_name = format!("Global\\{}", app_name);
        
        // 检查是否已有实例运行
        if Self::is_already_running(&mutex_name) {
            // 尝试激活现有窗口
            Self::activate_existing_instance();
            return Err("应用程序已在运行，已激活现有窗口".to_string());
        }

        Ok(SingleInstanceGuard {
            _mutex_name: mutex_name,
        })
    }

    fn is_already_running(mutex_name: &str) -> bool {
        use winapi::um::synchapi::CreateMutexA;
        use winapi::um::winbase::OpenMutexA;
        use winapi::um::winnt::SYNCHRONIZE;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;

        let c_mutex_name = CString::new(mutex_name).unwrap();
        
        unsafe {
            // 尝试打开现有的互斥量
            let existing_mutex = OpenMutexA(SYNCHRONIZE, FALSE, c_mutex_name.as_ptr());
            if !existing_mutex.is_null() {
                winapi::um::handleapi::CloseHandle(existing_mutex);
                return true;
            }

            // 创建新的互斥量
            let mutex = CreateMutexA(ptr::null_mut(), TRUE, c_mutex_name.as_ptr());
            if mutex.is_null() {
                return false;
            }

            let error = GetLastError();
            if error == ERROR_ALREADY_EXISTS {
                winapi::um::handleapi::CloseHandle(mutex);
                return true;
            }
        }
        
        false
    }

    fn activate_existing_instance() {
        unsafe {
            // 获取当前进程名称用于窗口查找
            let current_process_id = GetCurrentProcessId();
            TARGET_PROCESS_ID = current_process_id;
            FOUND_WINDOW = ptr::null_mut();

            // 枚举所有窗口查找我们的应用程序窗口
            EnumWindows(Some(enum_windows_proc), 0);

            if !FOUND_WINDOW.is_null() {
                Self::bring_window_to_front(FOUND_WINDOW);
            } else {
                // 如果没找到窗口，尝试通过窗口类名查找
                Self::find_and_activate_by_title();
            }
        }
    }

    fn find_and_activate_by_title() {
        unsafe {
            // 尝试查找包含"SCRCPY"的窗口标题
            let window_titles = [
                "SCRCPY 智能启动器",
                "scrcpy-launcher",
                "Auto-Scrcpy",
            ];

            for title in &window_titles {
                if let Ok(c_title) = CString::new(*title) {
                    let hwnd = FindWindowA(ptr::null(), c_title.as_ptr());
                    if !hwnd.is_null() {
                        // 验证这是我们的进程
                        let mut process_id: DWORD = 0;
                        GetWindowThreadProcessId(hwnd, &mut process_id);
                        
                        if process_id == GetCurrentProcessId() {
                            Self::bring_window_to_front(hwnd);
                            return;
                        }
                    }
                }
            }

            // 最后尝试：枚举所有窗口并检查进程ID
            TARGET_PROCESS_ID = GetCurrentProcessId();
            FOUND_WINDOW = ptr::null_mut();
            EnumWindows(Some(enum_windows_proc_by_process), 0);
            
            if !FOUND_WINDOW.is_null() {
                Self::bring_window_to_front(FOUND_WINDOW);
            }
        }
    }

    fn bring_window_to_front(hwnd: HWND) {
        unsafe {
            // 获取窗口线程ID
            let mut process_id: DWORD = 0;
            let window_thread_id = GetWindowThreadProcessId(hwnd, &mut process_id);
            let current_thread_id = GetCurrentThreadId();

            // 如果窗口被最小化，先还原它
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            } else {
                ShowWindow(hwnd, SW_SHOW);
            }

            // 附加线程输入，这样可以更好地激活窗口
            if window_thread_id != current_thread_id {
                AttachThreadInput(current_thread_id, window_thread_id, TRUE);
            }

            // 多重激活确保窗口到前台
            BringWindowToTop(hwnd);
            SetActiveWindow(hwnd);
            SetForegroundWindow(hwnd);

            // 分离线程输入
            if window_thread_id != current_thread_id {
                AttachThreadInput(current_thread_id, window_thread_id, FALSE);
            }

            // 再次确保窗口可见
            ShowWindow(hwnd, SW_SHOW);
        }
    }
}

// 窗口枚举回调函数
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut process_id: DWORD = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);

    // 检查是否是我们的进程
    if process_id == TARGET_PROCESS_ID {
        // 检查窗口标题是否包含我们的应用程序标识
        let mut title_buffer = [0u8; 256];
        let title_len = GetWindowTextA(hwnd, title_buffer.as_mut_ptr() as *mut i8, 256);
        
        if title_len > 0 {
            let title = String::from_utf8_lossy(&title_buffer[..title_len as usize]);
            // 检查是否是控制台窗口或我们的应用程序窗口
            if title.contains("SCRCPY") || title.contains("scrcpy-launcher") || 
               title.contains("Auto-Scrcpy") || title.contains("智能启动器") {
                FOUND_WINDOW = hwnd;
                return FALSE; // 停止枚举
            }
        }
    }

    TRUE // 继续枚举
}

// 按进程ID查找窗口的回调函数
unsafe extern "system" fn enum_windows_proc_by_process(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut process_id: DWORD = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);

    if process_id == TARGET_PROCESS_ID {
        // 获取窗口标题
        let mut title_buffer = [0u8; 256];
        let title_len = GetWindowTextA(hwnd, title_buffer.as_mut_ptr() as *mut i8, 256);
        
        if title_len > 0 {
            // 找到第一个有标题的窗口就认为是主窗口
            FOUND_WINDOW = hwnd;
            return FALSE; // 停止枚举
        }
    }

    TRUE // 继续枚举
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // 清理资源
    }
}