use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Show,
    Quit,
}

pub struct TrayHandle {
    rx: mpsc::Receiver<TrayEvent>,
    quitting: Arc<AtomicBool>,
    #[cfg(target_os = "windows")]
    hwnd: std::sync::Arc<std::sync::atomic::AtomicIsize>,
}

impl TrayHandle {
    pub fn start(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let quitting = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "windows")]
        let hwnd = win::start_tray_thread(ctx, tx, quitting.clone());
        #[cfg(not(target_os = "windows"))]
        let _ = (ctx, tx);

        Self {
            rx,
            quitting,
            #[cfg(target_os = "windows")]
            hwnd,
        }
    }

    pub fn poll(&self) -> Vec<TrayEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
        }
        out
    }

    pub fn mark_quitting(&self) {
        self.quitting.store(true, Ordering::SeqCst);
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    pub fn shutdown(&self) {
        #[cfg(target_os = "windows")]
        win::shutdown_tray(&self.hwnd);
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::TrayEvent;
    use eframe::egui;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::{mpsc, Arc};

    type HWND = isize;
    type HINSTANCE = isize;
    type HICON = isize;
    type HMENU = isize;
    type WPARAM = usize;
    type LPARAM = isize;
    type LRESULT = isize;
    type BOOL = i32;
    type UINT = u32;
    type DWORD = u32;

    const WM_DESTROY: UINT = 0x0002;
    const WM_CLOSE: UINT = 0x0010;
    const WM_NULL: UINT = 0x0000;
    const WM_COMMAND: UINT = 0x0111;
    const WM_USER: UINT = 0x0400;
    const WM_TRAYICON: UINT = WM_USER + 1;

    const WM_LBUTTONUP: LPARAM = 0x0202;
    const WM_LBUTTONDBLCLK: LPARAM = 0x0203;
    const WM_RBUTTONUP: LPARAM = 0x0205;
    const WM_CONTEXTMENU: LPARAM = 0x007B;

    const NIM_ADD: DWORD = 0x00000000;
    const NIM_DELETE: DWORD = 0x00000002;
    const NIF_MESSAGE: UINT = 0x00000001;
    const NIF_ICON: UINT = 0x00000002;
    const NIF_TIP: UINT = 0x00000004;

    const MF_STRING: UINT = 0x00000000;
    const TPM_RIGHTALIGN: UINT = 0x0008;
    const TPM_BOTTOMALIGN: UINT = 0x0020;

    const SW_SHOW: i32 = 5;
    const SW_RESTORE: i32 = 9;

    const ID_SHOW: usize = 1001;
    const ID_QUIT: usize = 1002;

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
        time: DWORD,
        pt: POINT,
    }

    #[repr(C)]
    struct WNDCLASSW {
        style: UINT,
        lpfn_wnd_proc: extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: HINSTANCE,
        h_icon: HICON,
        h_cursor: isize,
        hbr_background: isize,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
    }

    #[repr(C)]
    struct NOTIFYICONDATAW {
        cb_size: DWORD,
        h_wnd: HWND,
        u_id: UINT,
        u_flags: UINT,
        u_callback_message: UINT,
        h_icon: HICON,
        sz_tip: [u16; 128],
        dw_state: DWORD,
        dw_state_mask: DWORD,
        sz_info: [u16; 256],
        u_timeout_or_version: UINT,
        sz_info_title: [u16; 64],
        dw_info_flags: DWORD,
        guid_item: [u8; 16],
        h_balloon_icon: HICON,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> u16;
        fn CreateWindowExW(
            dwExStyle: DWORD,
            lpClassName: *const u16,
            lpWindowName: *const u16,
            dwStyle: DWORD,
            x: i32,
            y: i32,
            nWidth: i32,
            nHeight: i32,
            hWndParent: HWND,
            hMenu: HMENU,
            hInstance: HINSTANCE,
            lpParam: *mut core::ffi::c_void,
        ) -> HWND;
        fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
        fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
        fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
        fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
        fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
        fn PostQuitMessage(nExitCode: i32);
        fn DestroyWindow(hWnd: HWND) -> BOOL;
        fn CreatePopupMenu() -> HMENU;
        fn AppendMenuW(hMenu: HMENU, uFlags: UINT, uIDNewItem: usize, lpNewItem: *const u16) -> BOOL;
        fn DestroyMenu(hMenu: HMENU) -> BOOL;
        fn TrackPopupMenu(
            hMenu: HMENU,
            uFlags: UINT,
            x: i32,
            y: i32,
            nReserved: i32,
            hWnd: HWND,
            prcRect: *const core::ffi::c_void,
        ) -> BOOL;
        fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
        fn SetForegroundWindow(hWnd: HWND) -> BOOL;
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> HWND;
        fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        fn LoadIconW(hInstance: HINSTANCE, lpIconName: *const u16) -> HICON;
        fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
        fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> isize;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *const NOTIFYICONDATAW) -> BOOL;
        fn ExtractIconW(hInst: HINSTANCE, pszExeFileName: *const u16, nIconIndex: UINT) -> HICON;
    }

    const GWLP_USERDATA: i32 = -21;

    struct TrayContext {
        egui_ctx: egui::Context,
        tx: mpsc::Sender<TrayEvent>,
        quitting: Arc<AtomicBool>,
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn restore_main_window() {
        let title = to_wide("RDP 管理器");
        unsafe {
            let main_hwnd = FindWindowW(null(), title.as_ptr());
            if main_hwnd != 0 {
                ShowWindow(main_hwnd, SW_SHOW);
                ShowWindow(main_hwnd, SW_RESTORE);
                SetForegroundWindow(main_hwnd);
            }
        }
    }

    fn close_main_window() {
        let title = to_wide("RDP 管理器");
        unsafe {
            let main_hwnd = FindWindowW(null(), title.as_ptr());
            if main_hwnd != 0 {
                PostMessageW(main_hwnd, WM_CLOSE, 0, 0);
            }
        }
    }

    extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayContext;
            if !ptr.is_null() {
                let ctx = &*ptr;
                match msg {
                    WM_TRAYICON => {
                        let ev = lparam & 0xFFFF;
                        if ev == WM_LBUTTONUP || ev == WM_LBUTTONDBLCLK {
                            let _ = ctx.tx.send(TrayEvent::Show);
                            restore_main_window();
                            ctx.egui_ctx.request_repaint();
                            return 0;
                        }
                        if ev == WM_RBUTTONUP || ev == WM_CONTEXTMENU {
                            let menu = CreatePopupMenu();
                            if menu != 0 {
                                let open_txt = to_wide("打开主窗口");
                                let quit_txt = to_wide("退出");
                                AppendMenuW(menu, MF_STRING, ID_SHOW, open_txt.as_ptr());
                                AppendMenuW(menu, MF_STRING, ID_QUIT, quit_txt.as_ptr());
                                let mut pt = POINT { x: 0, y: 0 };
                                GetCursorPos(&mut pt);
                                SetForegroundWindow(hwnd);
                                TrackPopupMenu(
                                    menu,
                                    TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
                                    pt.x,
                                    pt.y,
                                    0,
                                    hwnd,
                                    null(),
                                );
                                PostMessageW(hwnd, WM_NULL, 0, 0);
                                DestroyMenu(menu);
                            }
                            return 0;
                        }
                    }
                    WM_COMMAND => {
                        let cmd = wparam & 0xFFFF;
                        if cmd == ID_SHOW {
                            let _ = ctx.tx.send(TrayEvent::Show);
                            restore_main_window();
                            ctx.egui_ctx.request_repaint();
                            return 0;
                        }
                        if cmd == ID_QUIT {
                            ctx.quitting.store(true, Ordering::SeqCst);
                            let _ = ctx.tx.send(TrayEvent::Quit);
                            ctx.egui_ctx.request_repaint();
                            close_main_window();
                            return 0;
                        }
                    }
                    WM_DESTROY => {
                        PostQuitMessage(0);
                        return 0;
                    }
                    _ => {}
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    pub fn start_tray_thread(
        egui_ctx: egui::Context,
        tx: mpsc::Sender<TrayEvent>,
        quitting: Arc<AtomicBool>,
    ) -> Arc<AtomicIsize> {
        let hwnd_holder = Arc::new(AtomicIsize::new(0));
        let hwnd_clone = hwnd_holder.clone();

        std::thread::spawn(move || unsafe {
            let h_instance = GetModuleHandleW(null());
            let class_name = to_wide("RdpManagerTrayClass");
            let wc = WNDCLASSW {
                style: 0,
                lpfn_wnd_proc: wnd_proc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance,
                h_icon: 0,
                h_cursor: 0,
                hbr_background: 0,
                lpsz_menu_name: null(),
                lpsz_class_name: class_name.as_ptr(),
            };
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                h_instance,
                null_mut(),
            );
            if hwnd == 0 {
                return;
            }
            hwnd_clone.store(hwnd, Ordering::SeqCst);

            let ctx_box = Box::new(TrayContext {
                egui_ctx,
                tx,
                quitting,
            });
            let ctx_ptr = Box::into_raw(ctx_box);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as isize);

            // Load embedded app icon from current executable, fallback to IDI_APPLICATION.
            let mut h_icon = 0;
            if let Ok(exe) = std::env::current_exe() {
                let exe_w = to_wide(&exe.to_string_lossy());
                h_icon = ExtractIconW(h_instance, exe_w.as_ptr(), 0);
            }
            if h_icon == 0 || h_icon == 1 {
                h_icon = LoadIconW(0, 32512 as *const u16); // IDI_APPLICATION
            }

            let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
            nid.cb_size = core::mem::size_of::<NOTIFYICONDATAW>() as DWORD;
            nid.h_wnd = hwnd;
            nid.u_id = 1;
            nid.u_flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.u_callback_message = WM_TRAYICON;
            nid.h_icon = h_icon;
            let tip = to_wide("RDP 管理器");
            let copy_len = tip.len().min(nid.sz_tip.len());
            nid.sz_tip[..copy_len].copy_from_slice(&tip[..copy_len]);

            Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg: MSG = core::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &nid);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ctx_ptr));
        });

        hwnd_holder
    }

    pub fn shutdown_tray(hwnd_holder: &AtomicIsize) {
        let hwnd = hwnd_holder.swap(0, Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                let mut nid: NOTIFYICONDATAW = core::mem::zeroed();
                nid.cb_size = core::mem::size_of::<NOTIFYICONDATAW>() as DWORD;
                nid.h_wnd = hwnd;
                nid.u_id = 1;
                Shell_NotifyIconW(NIM_DELETE, &nid);
                PostMessageW(hwnd, WM_DESTROY, 0, 0);
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}
