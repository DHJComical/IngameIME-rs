use std::char::decode_utf16;
use std::num::NonZeroIsize;
use std::slice::from_raw_parts;

use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Input::Ime::{
    ImmAssociateContext, ImmCreateContext, ImmDestroyContext, ImmGetCandidateListW, ImmGetCompositionStringW, ImmGetConversionStatus, ImmNotifyIME,
    ImmSetCandidateWindow, ImmSetCompositionWindow, ImmSetOpenStatus, CANDIDATEFORM, CANDIDATELIST,
    CFS_EXCLUDE, CFS_RECT, COMPOSITIONFORM, CPS_CANCEL,
    GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR,
    HIMC, IME_CMODE_NATIVE, IME_COMPOSITION_STRING, IME_CONVERSION_MODE,
    IMN_CHANGECANDIDATE, IMN_CLOSECANDIDATE, IMN_OPENCANDIDATE, IMN_SETCONVERSIONMODE,
    ISC_SHOWUICANDIDATEWINDOW, ISC_SHOWUICOMPOSITIONWINDOW, NI_COMPOSITIONSTR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetPropW, SetPropW, SetWindowLongPtrW, GWLP_WNDPROC,
    WM_IME_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY, WM_IME_SETCONTEXT,
    WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WM_INPUTLANGCHANGEREQUEST, WNDPROC,
};

use crate::interface::lib::{
    Candidate, CandidateCallback, CandidateConfig, CandidateEvent, CommitCallback, InputContext,
    InputMode, InputModeCallback, InputSourceCallback, InputSourceInfo, PreEdit, PreEditCallback,
    PreEditEvent,
};

fn log_info(msg: &str) {
    crate::interface::jni_api::log_info(msg);
}

fn log_debug(msg: &str) {
    crate::interface::jni_api::log_debug(msg);
}

fn log_error(msg: &str) {
    crate::interface::jni_api::log_info(&format!("ERROR: {}", msg));
}

fn log_warn(msg: &str) {
    crate::interface::jni_api::log_info(&format!("WARN: {}", msg));
}

#[allow(dead_code)]
#[allow(unused_assignments)]
unsafe extern "system" fn ingame_ime_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let handle = GetPropW(hwnd, w!("IngameIME_Userdata"));
        if handle.is_invalid() {
            // Context not set, use default window proc
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let context = &*(handle.0 as *const Imm32InputContext);

        match msg {
                WM_INPUTLANGCHANGEREQUEST => {
                    log_debug("WM_INPUTLANGCHANGEREQUEST");
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                WM_INPUTLANGCHANGE => {
                    log_debug("WM_INPUTLANGCHANGE");
                    // notify input source change
                    if let Some(cb) = &context.input_source_cb {
                        cb(context.get_input_source());
                    }
                    // notify input mode change
                    if let Some(cb) = &context.input_mode_cb {
                        cb(context.get_input_mode());
                    }
                    return LRESULT(1);
                }
                WM_IME_SETCONTEXT => {
                    log_debug("WM_SETCONTEXT");
                    let mut lparam = lparam.0;
                    lparam &= !ISC_SHOWUICOMPOSITIONWINDOW as isize;
                    if context.ui_less {
                        lparam &= !ISC_SHOWUICANDIDATEWINDOW as isize;
                    }
                    return DefWindowProcW(hwnd, msg, wparam, LPARAM(lparam));
                }
                WM_IME_STARTCOMPOSITION => {
                    log_debug("WM_IME_STARTCOMPOSITION");
                    // preedit event: begin
                    if let Some(cb) = &context.preedit_cb {
                        cb(PreEditEvent::Begin);
                    }
                    // Also fetch candidate list for new composition
                    if context.ui_less {
                        context.run_candidate_update();
                    }
                    return LRESULT(1);
                }
                WM_IME_COMPOSITION => {
                    log_debug("WM_IME_COMPOSITION");
                    // preedit event: update
                    if IME_COMPOSITION_STRING(lparam.0 as u32).contains(GCS_COMPSTR | GCS_CURSORPOS)
                    {
                        context.run_preedit_update();
                    }
                    // notify commit string
                    if IME_COMPOSITION_STRING(lparam.0 as u32).contains(GCS_RESULTSTR) {
                        context.run_commit();
                    }
                    // Also fetch candidate list on composition update
                    if context.ui_less {
                        context.run_candidate_update();
                    }
                    return LRESULT(1);
                }
                WM_IME_ENDCOMPOSITION => {
                    log_debug("WM_IME_ENDCOMPOSITION");
                    // preedit event: end
                    if let Some(cb) = &context.preedit_cb {
                        cb(PreEditEvent::End);
                    }
                    // Don't send CandidateEvent::End here as it would clear the candidate list
                    // when user selects a character and continues typing (e.g., "shenren" -> "神" -> "ren")
                    // The candidate list will be updated when new composition starts
                    return LRESULT(1);
                }
                WM_IME_NOTIFY => match wparam.0 as u32 {
                    IMN_OPENCANDIDATE => {
                        log_debug("IMN_OPENCANDIDATE");
                        // Don't send Begin event as it would clear the candidate list
                        // Candidate list will be fetched on IMN_CHANGECANDIDATE
                        return LRESULT(1);
                    }
                    IMN_CHANGECANDIDATE => {
                        log_debug("IMN_CHANGECANDIDATE");
                        if context.ui_less {
                            context.run_candidate_update();
                        }
                        return LRESULT(1);
                    }
                    IMN_CLOSECANDIDATE => {
                        log_debug("IMN_CLOSECANDIDATE");
                        if context.ui_less {
                            if let Some(cb) = &context.candidate_cb {
                                cb(CandidateEvent::End);
                            }
                        }
                        return LRESULT(1);
                    }
                    IMN_SETCONVERSIONMODE => {
                        log_debug("IMN_SETCONVERSIONMODE");
                        // notify input mode change
                        if let Some(cb) = &context.input_mode_cb {
                            cb(context.get_input_mode());
                        }
                        return LRESULT(1);
                    }
                    _ => {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }
                },
                WM_IME_CHAR => {
                    log_debug("WM_IME_CHAR");
                    // commit already handled in WM_IME_COMPOSITION, prevent from multiple conversion
                    return LRESULT(1);
                }
                // default
                _ => {
                    if let Some(proc) = context.proc {
                        return CallWindowProcW(proc, hwnd, msg, wparam, lparam);
                    } else {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }
                }
            }
    }
}

pub struct Imm32InputContext {
    hwnd: HWND,
    prev: HIMC,
    himc: HIMC,
    activated: bool,
    ui_less: bool,
    rect: RECT,
    proc: Option<WNDPROC>,
    commit_cb: Option<CommitCallback>,
    preedit_cb: Option<PreEditCallback>,
    candidate_cb: Option<CandidateCallback>,
    input_source_cb: Option<InputSourceCallback>,
    input_mode_cb: Option<InputModeCallback>,
    candidate_config: CandidateConfig,
}

impl Imm32InputContext {
    /// ui_less: whether to show candidate window or not(true: hide, false:show)
    pub fn new(hwnd: NonZeroIsize, ui_less: bool) -> Option<Box<dyn InputContext>> {
        log_info("Creating Imm32InputContext");
        unsafe {
            let hwnd: HWND = std::mem::transmute(hwnd);
            log_debug("Create HIMC");
            let himc = ImmCreateContext();
            if !himc.is_invalid() {
                log_debug("Associate NULL HIMC to disable IME");
                let prev = ImmAssociateContext(hwnd, HIMC::default());

                log_debug("Create context");
                let mut context = Box::new(Imm32InputContext {
                    hwnd,
                    prev,
                    himc,
                    activated: false,
                    ui_less,
                    rect: RECT::default(),
                    proc: None,
                    commit_cb: None,
                    preedit_cb: None,
                    candidate_cb: None,
                    input_source_cb: None,
                    input_mode_cb: None,
                    candidate_config: CandidateConfig::default(),
                });

                log_debug("Save userdata for WNDPROC");
                let ptr = &*context as *const Imm32InputContext as _;
                match SetPropW(hwnd, w!("IngameIME_Userdata"), Option::from(HANDLE(ptr))) {
                    Ok(_) => {
                        log_debug("Replace WNDPROC");
                        let proc_ptr = SetWindowLongPtrW(
                            hwnd,
                            GWLP_WNDPROC,
                            ingame_ime_proc as *const () as isize,
                        );
                        let proc: WNDPROC = std::mem::transmute(proc_ptr);
                        context.proc = Some(proc);

                        log_debug("Config OpenStatus");
                        if (!ImmSetOpenStatus(himc, true)).into() {
                            log_error("Unable to SetOpenStatus");
                            // Restore original WNDPROC
                            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc_ptr);
                            SetPropW(hwnd, w!("IngameIME_Userdata"), Option::from(HANDLE::default())).ok();
                            return None;
                        }
                        log_info("Imm32InputContext created");
                        Some(context)
                    }
                    Err(e) => {
                        log_error(&format!("Unable to SetPropW for IngameIME_Userdata: {}", e));
                        None
                    }
                }
            } else {
                log_error("Unable to create HIMC");
                None
            }
        }
    }

    fn run_preedit_update(&self) {
        unsafe {
            let size = ImmGetCompositionStringW(self.himc, GCS_COMPSTR, None, 0);
            if size > 0 {
                log_debug("Get Preedit");
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

                log_debug("Get Cursor Pos");
                let cursor = ImmGetCompositionStringW(self.himc, GCS_CURSORPOS, None, 0) as usize;

                log_debug("Notify PreEditEvent: Updated");
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
                log_debug("Get Commit String");
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

                log_debug("Notify CommitEvent");
                if let Some(cb) = &self.commit_cb {
                    cb(text);
                }
            }
        }
    }

    fn run_candidate_update(&self) {
        unsafe {
            let size = ImmGetCandidateListW(self.himc, 0, None, 0);
            
            let mut candidates = Vec::<String>::new();
            let mut selected: usize = 0;
            
            if size > 0 {
                log_debug("Get Candidates");
                let mut buffer = Vec::<u8>::with_capacity(size as usize);
                ImmGetCandidateListW(self.himc, 0, Some(buffer.as_mut_ptr() as _), size);

                let candidate = *(buffer.as_ptr() as *const CANDIDATELIST);

                // item count in the candidate list
                let items = candidate.dwPageSize as usize;

                // candidate strings
                let mut candidates_vec = Vec::<String>::with_capacity(items);

                // offset array
                let offsets = buffer.as_ptr().offset(6 * 4) as *const u32;

                // foreach string
                for i in 0..items {
                    // index of the string offset
                    let i_offset = (candidate.dwPageStart as usize + i) as isize;
                    // string offset
                    let offset = *offsets.offset(i_offset);
                    // string length in bytes
                    let len = if i + 1 < items {
                        *offsets.offset(i_offset + 1) - offset
                    } else {
                        size - offset
                    };
                    // string pointer
                    let ptr = buffer.as_ptr().offset(offset as isize) as *const u16;
                    let u16_slice = from_raw_parts(ptr, (len / 2) as usize);
                    let text: String = decode_utf16(u16_slice.iter().copied())
                        .map(|r| r.unwrap_or('�'))
                        .collect();
                    for part in text.split(char::is_whitespace) {
                        let trimmed = part.trim();
                        if !trimmed.is_empty() {
                            candidates_vec.push(trimmed.to_string());
                        }
                    }
                }

                log_debug(&format!(
                    "Parsed {} candidates from {} items",
                    candidates_vec.len(),
                    items
                ));
                for (i, c) in candidates_vec.iter().enumerate() {
                    log_debug(&format!("  [{}] {}", i, c));
                }

                // Apply max candidates limit
                let max_candidates = self.candidate_config.max_candidates;
                if candidates_vec.len() > max_candidates {
                    candidates_vec.truncate(max_candidates);
                    log_debug(&format!("Truncated to {} candidates", max_candidates));
                }

                // convert absolute pos to relative pos
                selected = (candidate.dwSelection - candidate.dwPageStart) as usize;
                selected = if selected < candidates_vec.len() {
                    selected
                } else {
                    0
                };
                
                candidates = candidates_vec;
            } else {
                log_debug("No candidates available (size=0)");
            }

            log_debug(&format!(
                "Sending {} candidates to callback, selected={}",
                candidates.len(),
                selected
            ));
            if let Some(cb) = &self.candidate_cb {
                cb(CandidateEvent::Update(Candidate {
                    candidates,
                    selected,
                }));
            }
        }
    }
}

impl InputContext for Imm32InputContext {
    fn get_input_source(&self) -> InputSourceInfo {
        InputSourceInfo::Unsupported
    }

    fn get_input_mode(&self) -> InputMode {
        unsafe {
            log_debug("Get InputMode");
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
                    log_info("Set Activated.");
                    // associate our himc to turn on ime
                    ImmAssociateContext(self.hwnd, self.himc);
                } else {
                    log_info("Set De-activated.");
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
        // empty space is not allowed
        let width = if width > 0 { width } else { 1 };
        let height = if height > 0 { height } else { 1 };
        let rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        if self.rect != rect {
            self.rect = rect;
            log_debug("Set CandidateWindow Pos");
            // candidate window
            unsafe {
                let mut candidate = CANDIDATEFORM::default();
                candidate.dwStyle = CFS_EXCLUDE;
                candidate.ptCurrentPos.x = x;
                candidate.ptCurrentPos.y = y;
                candidate.rcArea = rect;
                if (!ImmSetCandidateWindow(self.himc, &candidate)).into() {
                    log_error("Unable to SetCandidateWindow");
                }
            }
            log_debug("Set PreEditWindow Pos");
            unsafe {
                let mut composition = COMPOSITIONFORM::default();
                composition.dwStyle = CFS_RECT;
                composition.ptCurrentPos.x = x;
                composition.ptCurrentPos.y = y;
                composition.rcArea = rect;
                if (!ImmSetCompositionWindow(self.himc, &composition)).into() {
                    log_error("Unable to SetPreEditWindow");
                }
            }
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

    fn get_candidate_config(&self) -> CandidateConfig {
        self.candidate_config.clone()
    }

    fn set_candidate_config(&mut self, config: CandidateConfig) {
        self.candidate_config = config;
    }
}

impl Drop for Imm32InputContext {
    fn drop(&mut self) {
        unsafe {
            log_info("Dropping Imm32InputContext");
            // disable ime
            self.set_activated(false);
            // restore previous wndproc
            if let Some(proc) = self.proc {
                let proc_ptr: isize = std::mem::transmute(proc);
                SetWindowLongPtrW(self.hwnd, GWLP_WNDPROC, proc_ptr);
            }
            // clear pointer which will be invalid
            let _ = SetPropW(self.hwnd, w!("IngameIME_Userdata"), Option::from(HANDLE::default()))
                .inspect_err(|e| {
                    log_error(&format!("Unable to SetPropW for IngameIME_Userdata: {}", e));
                });
            // restore previous himc
            ImmAssociateContext(self.hwnd, self.prev);
            // destroy context
            if (!ImmDestroyContext(self.himc)).into() {
                log_error("Unable to destroy HIMC");
            }
            log_info("Imm32InputContext dropped");
        }
    }
}
