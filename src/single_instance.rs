use std::ffi::CString;
use std::ptr;
use winapi::shared::minwindef::{TRUE, FALSE};

pub struct SingleInstanceGuard {
    _mutex_name: String,
    handle: winapi::shared::ntdef::HANDLE,
}

impl SingleInstanceGuard {
    pub fn new(app_name: &str) -> Result<Self, String> {
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::synchapi::CreateMutexA;
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;

        let mutex_name = format!("Global\\{}", app_name);
        let c_mutex_name = CString::new(mutex_name.clone()).map_err(|_| "创建互斥量名称失败".to_string())?;

        unsafe {
            let handle = CreateMutexA(ptr::null_mut(), TRUE, c_mutex_name.as_ptr());
            if handle.is_null() {
                return Err("创建命名互斥量失败".to_string());
            }

            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return Err("应用程序已在运行".to_string());
            }

            Ok(SingleInstanceGuard { _mutex_name: mutex_name, handle })
        }
    }

    #[allow(dead_code)]
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

}


impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // 释放互斥量句柄
        unsafe {
            if !self.handle.is_null() {
                let _ = winapi::um::handleapi::CloseHandle(self.handle);
            }
        }
    }
}