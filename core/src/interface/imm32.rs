use std::ptr;

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::Ime::{
    CPS_CANCEL, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IME_COMPOSITION_STRING,
    IMN_CHANGECANDIDATE, IMN_CLOSECANDIDATE, IMN_OPENCANDIDATE, IMN_SETCONVERSIONMODE,
    ISC_SHOWUICANDIDATEWINDOW, ISC_SHOWUICOMPOSITIONWINDOW, ImmAssociateContext, ImmCreateContext,
    ImmDestroyContext, ImmNotifyIME, NI_COMPOSITIONSTR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, GetPropW, SetPropW, SetWindowLongPtrW,
    WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY, WM_IME_SETCONTEXT,
    WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WNDPROC,
};
use windows::core::w;

use crate::interface::lib::{CandidateEvent, PreEditEvent};

use super::lib::{
    CandidateCallback, InputContext, InputMethod, InputMethodCallback, InputMode,
    InputModeCallback, PreEditCallback,
};

#[allow(dead_code)]
unsafe extern "system" fn ingame_ime_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    mut lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let handle = GetPropW(hwnd, w!("IngameIME_Userdata"));
        if !handle.is_invalid() {
            let context = *(handle.0 as *const &Imm32InputContext);

            match msg {
                WM_INPUTLANGCHANGE => {
                    // notify input method change
                    if let Some(cb) = &context.input_method_cb {
                        cb(context.get_input_method());
                    }
                    // notify input mode change
                    if let Some(cb) = &context.input_mode_cb {
                        cb(context.get_input_mode());
                    }
                    return LRESULT(1);
                }
                WM_IME_SETCONTEXT => {
                    // hide preedit window and candidate window
                    lparam.0 &= !(ISC_SHOWUICOMPOSITIONWINDOW | ISC_SHOWUICANDIDATEWINDOW) as isize;
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                WM_IME_STARTCOMPOSITION => {
                    // preedit event: begin
                    if let Some(cb) = &context.preedit_cb {
                        cb(PreEditEvent::Begin);
                    }
                    return LRESULT(1);
                }
                WM_IME_COMPOSITION => {
                    // preedit event: update
                    if IME_COMPOSITION_STRING(lparam.0 as u32).contains(GCS_COMPSTR | GCS_CURSORPOS)
                    {
                        context.run_preedit_update();
                    }
                    // notify commit string
                    if IME_COMPOSITION_STRING(lparam.0 as u32).contains(GCS_RESULTSTR) {
                        context.run_commit();
                    }
                    return LRESULT(1);
                }
                WM_IME_ENDCOMPOSITION => {
                    // preedit event: end
                    if let Some(cb) = &context.preedit_cb {
                        cb(PreEditEvent::End);
                    }
                    // candidate event: end
                    if let Some(cb) = &context.candidate_cb {
                        cb(CandidateEvent::End);
                    }
                    return LRESULT(1);
                }
                WM_IME_NOTIFY => match wparam.0 as u32 {
                    IMN_OPENCANDIDATE => {
                        if let Some(cb) = &context.candidate_cb {
                            cb(CandidateEvent::Begin);
                        }
                        return LRESULT(1);
                    }
                    IMN_CHANGECANDIDATE => {
                        context.run_candidate_update();
                        return LRESULT(1);
                    }
                    IMN_CLOSECANDIDATE => {
                        if let Some(cb) = &context.candidate_cb {
                            cb(CandidateEvent::End);
                        }
                        return LRESULT(1);
                    }
                    IMN_SETCONVERSIONMODE => {
                        // notify input mode change
                        if let Some(cb) = &context.input_mode_cb {
                            cb(context.get_input_mode());
                        }
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }
                    _ => {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }
                },
                WM_IME_CHAR => {
                    // commit already handled in WM_IME_COMPOSITION, prevent from multiple conversion
                    return LRESULT(1);
                }
                // default
                _ => {
                    return CallWindowProcW(context.proc, hwnd, msg, wparam, lparam);
                }
            }
        }

        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
}

pub struct Imm32InputContext {
    hwnd: HWND,
    prev: HIMC,
    himc: HIMC,
    activated: bool,
    proc: WNDPROC,
    preedit_cb: Option<PreEditCallback>,
    candidate_cb: Option<CandidateCallback>,
    input_method_cb: Option<InputMethodCallback>,
    input_mode_cb: Option<InputModeCallback>,
}

impl Imm32InputContext {
    pub fn new(hwnd: HWND) -> Option<Box<dyn InputContext>> {
        unsafe {
            // create our own himc
            let himc = ImmCreateContext();
            if !himc.is_invalid() {
                // associate a NULL himc to disable ime
                let prev = ImmAssociateContext(hwnd, HIMC::default());

                // replace wndproc
                let proc: WNDPROC = std::mem::transmute(SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    ingame_ime_proc as isize,
                ));

                // create context
                let context = Box::new(Imm32InputContext {
                    hwnd,
                    prev,
                    himc,
                    activated: false,
                    proc,
                    preedit_cb: None,
                    candidate_cb: None,
                    input_method_cb: None,
                    input_mode_cb: None,
                });

                // save userdata for wndproc
                let _ = SetPropW(
                    hwnd,
                    w!("IngameIME_Userdata"),
                    std::mem::transmute(ptr::addr_of!(context) as u128),
                );

                Some(context)
            } else {
                None
            }
        }
    }

    fn run_preedit_update(&self) {}

    fn run_commit(&self) {}

    fn run_candidate_update(&self) {}
}

impl InputContext for Imm32InputContext {
    fn get_input_method(&self) -> InputMethod {
        InputMethod::Unsupported
    }

    fn get_input_mode(&self) -> InputMode {
        InputMode::Unsupported
    }

    fn get_activated(&self) -> bool {
        self.activated
    }

    fn set_activated(&mut self, activated: bool) {
        if activated != self.activated {
            self.activated = activated;
            unsafe {
                if self.activated {
                    // associate our himc to turn on ime
                    ImmAssociateContext(self.hwnd, self.himc);
                } else {
                    // notify ime that we are going to turn off
                    let _ = ImmNotifyIME(self.himc, NI_COMPOSITIONSTR, CPS_CANCEL, 0);
                    ImmAssociateContext(self.hwnd, HIMC::default());

                    // preedit event: end
                    if let Some(cb) = &self.preedit_cb {
                        cb(PreEditEvent::End);
                    }
                    // candidate event: end
                    if let Some(cb) = &self.candidate_cb {
                        cb(CandidateEvent::End);
                    }
                }
            }
        }
    }

    fn set_preedit_rect(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) {}

    fn set_preedit_callback(&mut self, callback: PreEditCallback) {
        self.preedit_cb = Some(callback);
    }
    fn set_candidate_callback(&mut self, callback: CandidateCallback) {
        self.candidate_cb = Some(callback);
    }
    fn set_input_method_callback(&mut self, callback: InputMethodCallback) {
        self.input_method_cb = Some(callback);
    }
    fn set_input_mode_callback(&mut self, callback: InputModeCallback) {
        self.input_mode_cb = Some(callback);
    }
}

impl Drop for Imm32InputContext {
    fn drop(&mut self) {
        unsafe {
            // disable ime
            self.set_activated(false);
            // restore previous wndproc
            SetWindowLongPtrW(self.hwnd, GWLP_WNDPROC, std::mem::transmute(self.proc));
            // clear pointer which will be invalid
            let _ = SetPropW(self.hwnd, w!("IngameIME_Userdata"), Some(HANDLE::default()));
            // restore previous himc
            ImmAssociateContext(self.hwnd, self.prev);
            // destroy context
            let _ = ImmDestroyContext(self.himc);
        }
    }
}
