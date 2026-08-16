//! 区域点击穿透：命中测试层（T5）。
//!
//! 核心机制：窗口始终接收鼠标事件；在 Windows 上子类化窗口过程，处理
//! [`WM_NCHITTEST`]，判断鼠标是否落在鲸鱼娘矩形内——命中返回 `HTCLIENT`（事件
//! 由本窗口处理），未命中返回 `HTTRANSPARENT`（事件穿透到下层窗口）。
//!
//! 坐标约定：
//! - [`HIT_REGION`] 存窗口逻辑坐标（CSS 像素），由前端 `getBoundingClientRect()`
//!   上报，与 `tauri::WebviewWindow` 的逻辑尺寸/位置一致。
//! - `WM_NCHITTEST` 的 `lParam` 是屏幕物理坐标：先 `ScreenToClient` 转窗口物理
//!   坐标，再除以 scale_factor 得到窗口逻辑坐标，与 [`HIT_REGION`] 比对。
//!   注意：这里假定 DPI 缩放各向同性且窗口位于单一显示器；跨显示器混合 DPI 的
//!   per-monitor 精确换算留待后续真机验证再处理。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 命中矩形（窗口逻辑坐标，CSS 像素）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// 点是否落在矩形内（含左/上边界，不含右/下边界）。
    #[inline]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// 共享命中区：`None` 表示无命中区（整窗穿透）。
static HIT_REGION: Mutex<Option<Rect>> = Mutex::new(None);

/// 设置命中区（窗口逻辑坐标，CSS 像素）。
pub fn set_hit_region(rect: Option<Rect>) {
    *HIT_REGION.lock().unwrap_or_else(|p| p.into_inner()) = rect;
}

/// 读取当前命中区。
pub fn get_hit_region() -> Option<Rect> {
    *HIT_REGION.lock().unwrap_or_else(|p| p.into_inner())
}

/// 初始化命中测试。Windows 子类化窗口过程；macOS/Linux 暂为 TODO/空实现。
pub fn init_hit_test(window: &tauri::WebviewWindow<tauri::Wry>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::init(window)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        log::warn!("[hit_test] macOS 命中测试尚未实现（TODO：NSView hitTest / NSTrackingArea）");
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = window;
        Ok(())
    }
}

/// 恢复原窗口过程（进程退出时调用，恢复子类化前的窗口过程）。
#[cfg(target_os = "windows")]
pub fn cleanup_hit_test() {
    win::cleanup();
}

#[cfg(target_os = "windows")]
mod win {
    use std::sync::Mutex;

    use tauri::WebviewWindow;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, HTCLIENT,
        HTTRANSPARENT, WM_NCHITTEST, WNDPROC,
    };

    /// 子类化所需的窗口状态。
    struct WndState {
        hwnd: isize,
        scale_factor: f64,
        original_proc: WNDPROC,
    }

    static WND_STATE: Mutex<Option<WndState>> = Mutex::new(None);

    /// 从 `lParam` 取屏幕坐标：低 16 位 x、高 16 位 y（符号扩展）。
    #[inline]
    fn lparam_to_screen(lparam: LPARAM) -> (i32, i32) {
        let lp = lparam.0 as i32;
        let x = (lp & 0xFFFF) as i16 as i32;
        let y = (lp >> 16) as i16 as i32;
        (x, y)
    }

    /// 自定义窗口过程：仅在 `WM_NCHITTEST` 做命中测试，其余消息原样链回原过程。
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCHITTEST {
            match super::get_hit_region() {
                None => return LRESULT(HTTRANSPARENT as isize),
                Some(rect) => {
                    let (sx, sy) = lparam_to_screen(lparam);
                    let mut pt = POINT { x: sx, y: sy };
                    // ScreenToClient 返回 BOOL（i32 别名），非 0 表示成功
                    if ScreenToClient(hwnd, &mut pt).as_bool() {
                        let scale = WND_STATE
                            .lock()
                            .ok()
                            .and_then(|s| s.as_ref().map(|s| s.scale_factor))
                            .unwrap_or(1.0)
                            .max(1.0);
                        // 物理像素 -> 逻辑像素，与前端 CSS 像素对齐
                        let lx = pt.x as f64 / scale;
                        let ly = pt.y as f64 / scale;
                        return if rect.contains(lx, ly) {
                            LRESULT(HTCLIENT as isize)
                        } else {
                            LRESULT(HTTRANSPARENT as isize)
                        };
                    }
                }
            }
        }

        // 链回原窗口过程；状态缺失时退回到 DefWindowProcW
        let original = WND_STATE
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.original_proc));
        match original {
            Some(Some(proc)) => CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// 子类化窗口过程，启用命中测试穿透。
    pub fn init(window: &WebviewWindow<tauri::Wry>) -> Result<(), String> {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;

        let mut state = WND_STATE.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = state.as_mut() {
            // 已子类化：仅刷新 scale_factor（hwnd 不变）
            existing.scale_factor = scale_factor;
            return Ok(());
        }

        // 函数项先 coerce 成函数指针，避免“函数项直接转整数”的警告
        let new_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = wnd_proc;
        // SetWindowLongPtrW 返回之前的值（即原窗口过程地址）；0 视为失败
        let original_addr =
            unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, new_proc as usize as isize) };
        if original_addr == 0 {
            return Err("SetWindowLongPtrW 子类化失败（返回 0）".to_string());
        }
        // 非 0 地址 transmute 回函数指针，必为 Some
        let original_proc: WNDPROC = unsafe { std::mem::transmute(original_addr) };
        log::info!(
            "[hit_test] 已子类化窗口 HWND={hwnd:?} scale_factor={scale_factor:.2} original_proc={original_addr:#x}"
        );
        *state = Some(WndState {
            hwnd: hwnd.0 as isize,
            scale_factor,
            original_proc,
        });
        Ok(())
    }

    /// 恢复原窗口过程。
    pub fn cleanup() {
        let state = WND_STATE.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(s) = state {
            unsafe {
                SetWindowLongPtrW(
                    HWND(s.hwnd as *mut core::ffi::c_void),
                    GWLP_WNDPROC,
                    s.original_proc.expect("原窗口过程不可为空") as usize as isize,
                );
            }
            log::info!("[hit_test] 已恢复原窗口过程");
        }
    }
}
