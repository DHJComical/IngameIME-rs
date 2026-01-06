use std::char::decode_utf16;
use std::num::NonZeroIsize;
use std::slice::from_raw_parts;

use log::{debug, error, info, warn};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CANDIDATELIST, CFS_EXCLUDE, CFS_RECT, COMPOSITIONFORM, CPS_CANCEL, GCS_COMPSTR,
    GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IME_CMODE_NATIVE, IME_COMPOSITION_STRING,
    IME_CONVERSION_MODE, IMN_CHANGECANDIDATE, IMN_CLOSECANDIDATE, IMN_OPENCANDIDATE,
    IMN_SETCONVERSIONMODE, ISC_SHOWUICANDIDATEWINDOW, ISC_SHOWUICOMPOSITIONWINDOW,
    ImmAssociateContext, ImmCreateContext, ImmDestroyContext, ImmGetCandidateListW,
    ImmGetCompositionStringW, ImmGetConversionStatus, ImmNotifyIME, ImmSetCandidateWindow,
    ImmSetCompositionWindow, NI_COMPOSITIONSTR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, GetPropW, SetPropW, SetWindowLongPtrW,
    WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY, WM_IME_SETCONTEXT,
    WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WNDPROC,
};
use windows::core::w;

use crate::interface::lib::{
    Candidate, CandidateCallback, CandidateEvent, CommitCallback, InputContext, InputMode,
    InputModeCallback, InputSourceCallback, InputSourceInfo, PreEdit, PreEditCallback,
    PreEditEvent,
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
            let context = &*(handle.0 as *const Imm32InputContext);

            match msg {
                WM_INPUTLANGCHANGE => {
                    debug!("WM_INPUTLANGCHANGE");
                    // notify input method change
                    if let Some(cb) = &context.input_source_cb {
                        cb(context.get_input_method());
                    }
                    // notify input mode change
                    if let Some(cb) = &context.input_mode_cb {
                        cb(context.get_input_mode());
                    }
                    return LRESULT(1);
                }
                WM_IME_SETCONTEXT => {
                    debug!("WM_SETCONTEXT");
                    // hide preedit window and candidate window
                    lparam.0 &= !(ISC_SHOWUICOMPOSITIONWINDOW | ISC_SHOWUICANDIDATEWINDOW) as isize;
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                WM_IME_STARTCOMPOSITION => {
                    debug!("WM_IME_STARTCOMPOSITION");
                    // preedit event: begin
                    if let Some(cb) = &context.preedit_cb {
                        cb(PreEditEvent::Begin);
                    }
                    return LRESULT(1);
                }
                WM_IME_COMPOSITION => {
                    debug!("WM_IME_COMPOSITION");
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
                    debug!("WM_IME_ENDCOMPOSITION");
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
                        debug!("IMN_OPENCANDIDATE");
                        if let Some(cb) = &context.candidate_cb {
                            cb(CandidateEvent::Begin);
                        }
                        return LRESULT(1);
                    }
                    IMN_CHANGECANDIDATE => {
                        debug!("IMN_CHANGECANDIDATE");
                        context.run_candidate_update();
                        return LRESULT(1);
                    }
                    IMN_CLOSECANDIDATE => {
                        debug!("IMN_CLOSECANDIDATE");
                        if let Some(cb) = &context.candidate_cb {
                            cb(CandidateEvent::End);
                        }
                        return LRESULT(1);
                    }
                    IMN_SETCONVERSIONMODE => {
                        debug!("IMN_SETCONVERSIONMODE");
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
                    debug!("WM_IME_CHAR");
                    // commit already handled in WM_IME_COMPOSITION, prevent from multiple conversion
                    return LRESULT(1);
                }
                // default
                _ => {
                    return CallWindowProcW(context.proc, hwnd, msg, wparam, lparam);
                }
            }
        }

        warn!("Unable to GetPropW for IngameIME_Userdata");
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
}

pub struct Imm32InputContext {
    hwnd: HWND,
    prev: HIMC,
    himc: HIMC,
    activated: bool,
    proc: WNDPROC,
    commit_cb: Option<CommitCallback>,
    preedit_cb: Option<PreEditCallback>,
    candidate_cb: Option<CandidateCallback>,
    input_source_cb: Option<InputSourceCallback>,
    input_mode_cb: Option<InputModeCallback>,
}

impl Imm32InputContext {
    pub fn new(hwnd: NonZeroIsize) -> Option<Box<dyn InputContext>> {
        info!("Creating Imm32InputContext");
        unsafe {
            let hwnd: HWND = std::mem::transmute(hwnd);
            debug!("Create HIMC");
            let himc = ImmCreateContext();
            if !himc.is_invalid() {
                debug!("Associate NULL HIMC to disable IME");
                let prev = ImmAssociateContext(hwnd, HIMC::default());

                debug!("Replace WNDPROC");
                let proc: WNDPROC = std::mem::transmute(SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    ingame_ime_proc as isize,
                ));

                debug!("Create context");
                let context = Box::new(Imm32InputContext {
                    hwnd,
                    prev,
                    himc,
                    activated: false,
                    proc,
                    commit_cb: None,
                    preedit_cb: None,
                    candidate_cb: None,
                    input_source_cb: None,
                    input_mode_cb: None,
                });

                debug!("Save userdata for WNDPROC");
                let ptr = &*context as *const Imm32InputContext as _;
                match SetPropW(hwnd, w!("IngameIME_Userdata"), Some(HANDLE(ptr))) {
                    Ok(_) => {
                        info!("Imm32InputContext has created");
                        Some(context)
                    }
                    Err(e) => {
                        error!("Unable to SetPropW for IngameIME_Userdata: {e}");
                        None
                    }
                }
            } else {
                error!("Unable to create HIMC");
                None
            }
        }
    }

    fn run_preedit_update(&self) {
        unsafe {
            let size = ImmGetCompositionStringW(self.himc, GCS_COMPSTR, None, 0);
            if size > 0 {
                debug!("Get Preedit");
                let mut buffer = Vec::<u8>::with_capacity(size as usize);
                ImmGetCompositionStringW(
                    self.himc,
                    GCS_COMPSTR,
                    Some(buffer.as_mut_ptr() as _),
                    size as u32,
                );
                let u16_slice = from_raw_parts(buffer.as_ptr() as *const u16, (size / 2) as usize);
                let text: String = decode_utf16(u16_slice.iter().copied())
                    .map(|r| r.unwrap_or('�'))
                    .collect();

                debug!("Get Cursor Pos");
                let cursor = ImmGetCompositionStringW(self.himc, GCS_CURSORPOS, None, 0) as usize;

                debug!("Notify PreEditEvent: Updated");
                if let Some(cb) = &self.preedit_cb {
                    cb(PreEditEvent::Update(PreEdit { text, cursor }));
                }
            }
        }
    }

    fn run_commit(&self) {
        unsafe {
            let size = ImmGetCompositionStringW(self.himc, GCS_RESULTSTR, None, 0);
            if size > 0 {
                debug!("Get Commit String");
                let mut buffer = Vec::<u8>::with_capacity(size as usize);
                ImmGetCompositionStringW(
                    self.himc,
                    GCS_RESULTSTR,
                    Some(buffer.as_mut_ptr() as _),
                    size as u32,
                );
                let u16_slice = from_raw_parts(buffer.as_ptr() as *const u16, (size / 2) as usize);
                let text: String = decode_utf16(u16_slice.iter().copied())
                    .map(|r| r.unwrap_or('�'))
                    .collect();

                debug!("Notify CommitEvent");
                if let Some(cb) = &self.commit_cb {
                    cb(text);
                }
            }
        }
    }

    fn run_candidate_update(&self) {
        unsafe {
            let size = ImmGetCandidateListW(self.himc, 0, None, 0);
            if size > 0 {
                debug!("Get Candidates");
                let mut buffer = Vec::<u8>::with_capacity(size as usize);
                ImmGetCandidateListW(self.himc, 0, Some(buffer.as_mut_ptr() as _), size);

                let candidate = *(buffer.as_ptr() as *const CANDIDATELIST);

                // item count in the candidate list
                let items = candidate.dwPageSize as usize;

                // candidate strings
                let mut candidates = Vec::<String>::with_capacity(items);

                // foreach string
                for i in 0..items {
                    // index of the string offset
                    let i_offset = candidate.dwPageStart as usize + i;
                    // string offset
                    let offset = candidate.dwOffset[i_offset];
                    // string length in bytes
                    let len = if i + 1 < items {
                        candidate.dwOffset[i_offset + 1] - offset
                    } else {
                        size - offset
                    };
                    // string pointer
                    let ptr = buffer.as_ptr().offset(offset as isize) as *const u16;
                    let u16_slice = from_raw_parts(ptr, (len / 2) as usize);
                    let text: String = decode_utf16(u16_slice.iter().copied())
                        .map(|r| r.unwrap_or('�'))
                        .collect();
                    candidates.push(text);
                }

                // convert absolute pos to relative pos
                let selected = (candidate.dwSelection - candidate.dwPageStart) as usize;

                debug!("Notify CandidateEvent: Update");
                if let Some(cb) = &self.candidate_cb {
                    cb(CandidateEvent::Update(Candidate {
                        candidates,
                        selected,
                    }));
                }
            }
        }
    }
}

impl InputContext for Imm32InputContext {
    fn get_input_method(&self) -> InputSourceInfo {
        InputSourceInfo::Unsupported
    }

    fn get_input_mode(&self) -> InputMode {
        unsafe {
            debug!("Get InputMode");
            let mut mode = IME_CONVERSION_MODE(0);
            let _ = ImmGetConversionStatus(self.himc, Some(&mut mode as *mut _), None);
            if mode.contains(IME_CMODE_NATIVE) {
                InputMode::Native
            } else {
                InputMode::Alpha
            }
        }
    }

    fn get_activated(&self) -> bool {
        self.activated
    }

    fn set_activated(&mut self, activated: bool) {
        if activated != self.activated {
            self.activated = activated;
            unsafe {
                if self.activated {
                    info!("Set Activated.");
                    // associate our himc to turn on ime
                    ImmAssociateContext(self.hwnd, self.himc);
                } else {
                    info!("Set De-activated.");
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

    fn set_preedit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug!("Set CandidateWindow Pos");
        // candidate window
        unsafe {
            let mut candidate = CANDIDATEFORM::default();
            candidate.dwStyle = CFS_EXCLUDE;
            candidate.ptCurrentPos.x = x;
            candidate.ptCurrentPos.y = y;
            candidate.rcArea = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = ImmSetCandidateWindow(self.himc, &candidate);
        }
        debug!("Set PreEditWindow Pos");
        unsafe {
            let mut composition = COMPOSITIONFORM::default();
            composition.dwStyle = CFS_RECT;
            composition.ptCurrentPos.x = x;
            composition.ptCurrentPos.y = y;
            composition.rcArea = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = ImmSetCompositionWindow(self.himc, &composition);
        }
    }

    fn set_commit_callback(&mut self, callback: CommitCallback) {
        self.commit_cb = Some(callback);
    }

    fn set_preedit_callback(&mut self, callback: PreEditCallback) {
        self.preedit_cb = Some(callback);
    }

    fn set_candidate_callback(&mut self, callback: CandidateCallback) {
        self.candidate_cb = Some(callback);
    }

    fn set_input_source_callback(&mut self, callback: InputSourceCallback) {
        self.input_source_cb = Some(callback);
    }

    fn set_input_mode_callback(&mut self, callback: InputModeCallback) {
        self.input_mode_cb = Some(callback);
    }
}

impl Drop for Imm32InputContext {
    fn drop(&mut self) {
        unsafe {
            info!("Dropping Imm32InputContext");
            // disable ime
            self.set_activated(false);
            // restore previous wndproc
            SetWindowLongPtrW(self.hwnd, GWLP_WNDPROC, std::mem::transmute(self.proc));
            // clear pointer which will be invalid
            let _ = SetPropW(self.hwnd, w!("IngameIME_Userdata"), Some(HANDLE::default()))
                .inspect_err(|e| {
                    error!("Unable to SetPropW for IngameIME_Userdata: {e}");
                });
            // restore previous himc
            ImmAssociateContext(self.hwnd, self.prev);
            // destroy context
            if (!ImmDestroyContext(self.himc)).into() {
                error!("Unable to destroy HIMC");
            }
            info!("Imm32InputContext dropped");
        }
    }
}
