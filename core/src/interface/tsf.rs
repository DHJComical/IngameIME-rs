//! TSF (Text Services Framework) implementation for IngameIME-rs

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::cell::RefCell;
use std::char::decode_utf16;
use std::sync::atomic::AtomicU32;

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::MapWindowPoints,
    Win32::System::Com::*,
    Win32::UI::TextServices::*,
    Win32::UI::WindowsAndMessaging::GetWindowRect,
};

// IID constants for TSF interfaces
const IID_ITfTextEditSink: GUID = GUID::from_u128(0x0491dd00_1172_4b71_84aa_8f4b3aeb517b);
const IID_ITfUIElementSink: GUID = GUID::from_u128(0xea1680b9_0c0d_4d8d_b6e5_89a02b7dd6b0);
const IID_ITfCompartmentEventSink: GUID = GUID::from_u128(0x25f36c01_2a81_4e67_8d30_5f76d7cc5b07);
const IID_ITfThreadMgr: GUID = GUID::from_u128(0xaa80e801_2021_11d2_93e0_00609778470c);
const IID_ITfContextOwner: GUID = GUID::from_u128(0x96eb9ce0_90e0_4c39_87c0_6e4d5e066226);

// TSF constants
const TF_INVALID_UIELEMENTID: u32 = 0xffffffff;

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

fn to_utf8(wide: &[u16]) -> String {
    decode_utf16(wide.iter().copied())
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct PreEditRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PreEditRect {
    pub fn to_rect(&self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }
}

// ============================================================================
// ContextOwner - Implements ITfContextOwner
// ============================================================================

#[implement(ITfContextOwner)]
pub struct ContextOwner {
    ref_count: AtomicU32,
    input_ctx: *mut TsInputContextInner,
    cookie: RefCell<u32>,
}

impl ContextOwner {
    pub fn new(input_ctx: *mut TsInputContextInner) -> Self {
        Self {
            ref_count: AtomicU32::new(1),
            input_ctx,
            cookie: RefCell::new(TF_INVALID_COOKIE),
        }
    }

    fn get_rect(&self) -> PreEditRect {
        unsafe { (*self.input_ctx).rect }
    }

    fn get_hwnd(&self) -> HWND {
        unsafe { (*self.input_ctx).hwnd }
    }

    pub fn unadvise(&self, ctx: &ITfContext) {
        unsafe {
            let cookie = *self.cookie.borrow();
            if cookie != TF_INVALID_COOKIE {
                if let Ok(source) = ctx.cast::<ITfSource>() {
                    let _ = source.UnadviseSink(cookie);
                }
                *self.cookie.borrow_mut() = TF_INVALID_COOKIE;
            }
        }
    }
}

impl ITfContextOwner_Impl for ContextOwner_Impl {
    fn GetACPFromPoint(&self, _ptscreen: *const POINT, _dwflags: u32) -> Result<i32> {
        Err(Error::from_win32())
    }

    fn GetTextExt(&self, _acpstart: i32, _acpend: i32, prc: *mut RECT, pfclipped: *mut BOOL) -> Result<()> {
        unsafe {
            if prc.is_null() {
                return Err(Error::from_win32());
            }
            *prc = self.get_rect().to_rect();
            // Map window coordinates to screen coordinates
            let hwnd = self.get_hwnd();
            let _ = MapWindowPoints(hwnd, None, core::slice::from_raw_parts_mut(prc as *mut POINT, 2));
            if !pfclipped.is_null() {
                *pfclipped = BOOL(0);
            }
        }
        Ok(())
    }

    fn GetScreenExt(&self) -> Result<RECT> {
        unsafe {
            let mut rect = RECT::default();
            GetWindowRect(self.get_hwnd(), &mut rect)?;
            Ok(rect)
        }
    }

    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(TS_STATUS {
            dwDynamicFlags: 0,
            dwStaticFlags: 0,
        })
    }

    fn GetWnd(&self) -> Result<HWND> {
        Ok(self.get_hwnd())
    }

    fn GetAttribute(&self, _rguidattribute: *const GUID) -> Result<VARIANT> {
        Ok(VARIANT::default())
    }
}

// ============================================================================
// CompositionHandler - Implements ITfContextOwnerCompositionSink, ITfTextEditSink, ITfUIElementSink
// ============================================================================

#[implement(ITfContextOwnerCompositionSink, ITfTextEditSink, ITfUIElementSink)]
pub struct CompositionHandler {
    ref_count: AtomicU32,
    input_ctx: *mut TsInputContextInner,
    comp_view: RefCell<Option<ITfRangeACP>>,
    ele_mgr: RefCell<Option<ITfUIElementMgr>>,
    ele: RefCell<Option<ITfUIElement>>,
    ele_id: RefCell<u32>,
    cookie_ele: RefCell<u32>,
    cookie_edit: RefCell<u32>,
}

impl CompositionHandler {
    pub fn new(input_ctx: *mut TsInputContextInner) -> Self {
        Self {
            ref_count: AtomicU32::new(1),
            input_ctx,
            comp_view: RefCell::new(None),
            ele_mgr: RefCell::new(None),
            ele: RefCell::new(None),
            ele_id: RefCell::new(TF_INVALID_UIELEMENTID),
            cookie_ele: RefCell::new(TF_INVALID_COOKIE),
            cookie_edit: RefCell::new(TF_INVALID_COOKIE),
        }
    }

    pub fn initialize(&self, ctx: &ITfContext, _client_id: u32) -> Result<()> {
        unsafe {
            let inner = &*self.input_ctx;

            if inner.ui_less {
                if let Ok(thread_mgr) = inner.thread_mgr.as_ref().unwrap().cast::<ITfUIElementMgr>() {
                    let source: ITfSource = thread_mgr.cast()?;
                    let unknown: IUnknown = self.cast()?;
                    let cookie = source.AdviseSink(&IID_ITfUIElementSink, &unknown)?;
                    *self.cookie_ele.borrow_mut() = cookie;
                    *self.ele_mgr.borrow_mut() = Some(thread_mgr);
                }
            }

            let source: ITfSource = ctx.cast()?;
            let unknown: IUnknown = self.cast()?;
            let cookie = source.AdviseSink(&IID_ITfTextEditSink, &unknown)?;
            *self.cookie_edit.borrow_mut() = cookie;

            Ok(())
        }
    }

    pub fn unadvise_sinks(&self, ctx: &ITfContext) {
        unsafe {
            let cookie_ele = *self.cookie_ele.borrow();
            if cookie_ele != TF_INVALID_COOKIE {
                if let Some(source) = self.ele_mgr.borrow().as_ref().and_then(|m| m.cast::<ITfSource>().ok()) {
                    let _ = source.UnadviseSink(cookie_ele);
                }
                *self.cookie_ele.borrow_mut() = TF_INVALID_COOKIE;
            }

            let cookie_edit = *self.cookie_edit.borrow();
            if cookie_edit != TF_INVALID_COOKIE {
                if let Ok(source) = ctx.cast::<ITfSource>() {
                    let _ = source.UnadviseSink(cookie_edit);
                }
                *self.cookie_edit.borrow_mut() = TF_INVALID_COOKIE;
            }
        }
    }

    fn run_preedit_begin(&self) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).preedit_cb {
                cb(PreEditEvent::Begin);
            }
        }
    }

    fn run_preedit_update(&self, text: String, cursor: usize) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).preedit_cb {
                cb(PreEditEvent::Update(PreEdit { text, cursor }));
            }
        }
    }

    fn run_preedit_end(&self) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).preedit_cb {
                cb(PreEditEvent::End);
            }
        }
    }

    fn run_commit(&self, text: String) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).commit_cb {
                cb(text);
            }
        }
    }

    fn run_candidate_begin(&self) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).candidate_cb {
                cb(CandidateEvent::Begin);
            }
        }
    }

    fn run_candidate_update(&self, candidates: Vec<String>, selected: usize) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).candidate_cb {
                cb(CandidateEvent::Update(Candidate { candidates, selected }));
            }
        }
    }

    fn run_candidate_end(&self) {
        unsafe {
            if let Some(cb) = &(*self.input_ctx).candidate_cb {
                cb(CandidateEvent::End);
            }
        }
    }
}

impl ITfContextOwnerCompositionSink_Impl for CompositionHandler_Impl {
    fn OnStartComposition(&self, pcomposition: Option<&ITfCompositionView>) -> Result<BOOL> {
        log_debug("OnStartComposition");
        if let Some(comp) = pcomposition {
            if let Ok(range) = comp.cast::<ITfRangeACP>() {
                *self.comp_view.borrow_mut() = Some(range);
            }
        }
        self.run_preedit_begin();
        Ok(BOOL(1))
    }

    fn OnUpdateComposition(&self, pcomposition: Option<&ITfCompositionView>, _prangenew: Option<&ITfRange>) -> Result<()> {
        log_debug("OnUpdateComposition");
        if let Some(comp) = pcomposition {
            if let Ok(range) = comp.cast::<ITfRangeACP>() {
                *self.comp_view.borrow_mut() = Some(range);
            }
        }
        Ok(())
    }

    fn OnEndComposition(&self, _pcomposition: Option<&ITfCompositionView>) -> Result<()> {
        log_debug("OnEndComposition");
        *self.comp_view.borrow_mut() = None;
        self.run_preedit_end();
        Ok(())
    }
}

impl ITfTextEditSink_Impl for CompositionHandler_Impl {
    fn OnEndEdit(&self, pic: Option<&ITfContext>, ec: u32, _peditrecord: Option<&ITfEditRecord>) -> Result<()> {
        log_debug("OnEndEdit");
        unsafe {
            let Some(ctx) = pic else { return Ok(()); };
            if self.comp_view.borrow().is_none() {
                return Ok(());
            }

            if let Some(range) = self.comp_view.borrow().as_ref() {
                let (mut acp_start, mut len) = (0, 0);
                if range.GetExtent(&mut acp_start, &mut len).is_ok() && len > 0 {
                    let mut selections = [TF_SELECTION::default()];
                    let mut fetched = 0u32;
                    if ctx.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched).is_ok() && fetched > 0 {
                        let text = format!("[{} chars]", len);
                        self.run_preedit_update(text, acp_start as usize);
                    }
                }
            }
        }
        Ok(())
    }
}

impl ITfUIElementSink_Impl for CompositionHandler_Impl {
    fn BeginUIElement(&self, dwuielementid: u32, pbshow: *mut BOOL) -> Result<()> {
        log_debug(&format!("BeginUIElement: {}", dwuielementid));
        unsafe {
            if pbshow.is_null() {
                return Err(Error::from_win32());
            }
            *pbshow = BOOL(0);

            let inner = &*self.input_ctx;
            if !inner.ui_less {
                return Ok(());
            }

            if dwuielementid == TF_INVALID_UIELEMENTID {
                return Ok(());
            }

            if let Some(ref ele_mgr) = *self.ele_mgr.borrow() {
                if let Ok(ui_ele) = ele_mgr.GetUIElement(dwuielementid) {
                    *self.ele.borrow_mut() = Some(ui_ele);
                    *self.ele_id.borrow_mut() = dwuielementid;
                    self.run_candidate_begin();
                }
            }
        }
        Ok(())
    }

    fn UpdateUIElement(&self, dwuielementid: u32) -> Result<()> {
        log_debug(&format!("UpdateUIElement: {}", dwuielementid));
        unsafe {
            let ele_id = *self.ele_id.borrow();
            if ele_id == TF_INVALID_UIELEMENTID || dwuielementid != ele_id {
                return Ok(());
            }

            if let Some(ref ele) = *self.ele.borrow() {
                if let Ok(cand_ele) = ele.cast::<ITfCandidateListUIElement>() {
                    let count: u32 = cand_ele.GetCount()?;
                    let mut page_count: u32 = 0;
                    cand_ele.GetPageIndex(&mut [], &mut page_count)?;

                    let mut page_starts = vec![0u32; page_count as usize];
                    cand_ele.GetPageIndex(&mut page_starts, &mut page_count)?;

                    let cur_page: u32 = cand_ele.GetCurrentPage()?;
                    let page_start = page_starts[cur_page as usize] as usize;
                    let page_end = if cur_page as usize + 1 < page_starts.len() {
                        page_starts[cur_page as usize + 1] as usize
                    } else {
                        count as usize
                    };

                    let sel: u32 = cand_ele.GetSelection()?;
                    let rel_sel = (sel as usize - page_start) as usize;

                    let mut candidates = Vec::new();
                    for i in page_start..page_end {
                        match cand_ele.GetString(i as u32) {
                            Ok(bstr) => candidates.push(bstr.to_string()),
                            Err(_) => candidates.push("[err]".to_string()),
                        }
                    }

                    self.run_candidate_update(candidates, rel_sel);
                }
            }
        }
        Ok(())
    }

    fn EndUIElement(&self, dwuielementid: u32) -> Result<()> {
        log_debug(&format!("EndUIElement: {}", dwuielementid));
        let ele_id = *self.ele_id.borrow();
        if ele_id == TF_INVALID_UIELEMENTID || dwuielementid != ele_id {
            return Ok(());
        }
        *self.ele_id.borrow_mut() = TF_INVALID_UIELEMENTID;
        *self.ele.borrow_mut() = None;
        self.run_candidate_end();
        Ok(())
    }
}

// ============================================================================
// InputModeHandler - Implements ITfCompartmentEventSink
// ============================================================================

#[implement(ITfCompartmentEventSink)]
pub struct InputModeHandler {
    ref_count: AtomicU32,
    input_ctx: *mut TsInputContextInner,
    comp_mgr: RefCell<Option<ITfCompartmentMgr>>,
    mode: RefCell<Option<ITfCompartment>>,
    cookie: RefCell<u32>,
    input_mode: RefCell<InputMode>,
}

impl InputModeHandler {
    pub fn new(input_ctx: *mut TsInputContextInner) -> Self {
        Self {
            ref_count: AtomicU32::new(1),
            input_ctx,
            comp_mgr: RefCell::new(None),
            mode: RefCell::new(None),
            cookie: RefCell::new(TF_INVALID_COOKIE),
            input_mode: RefCell::new(InputMode::Alpha),
        }
    }

    pub fn initialize(&self, thread_mgr: &ITfThreadMgr) -> Result<()> {
        unsafe {
            let comp_mgr: ITfCompartmentMgr = thread_mgr.cast()?;
            *self.comp_mgr.borrow_mut() = Some(comp_mgr.clone());

            let mode = comp_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)?;
            *self.mode.borrow_mut() = Some(mode.clone());

            // 获取初始模式 - 简化处理，默认 Alpha
            *self.input_mode.borrow_mut() = InputMode::Alpha;

            let source: ITfSource = mode.cast()?;
            let unknown: IUnknown = self.cast()?;
            let cookie = source.AdviseSink(&IID_ITfCompartmentEventSink, &unknown)?;
            *self.cookie.borrow_mut() = cookie;

            Ok(())
        }
    }

    pub fn unadvise_sink(&self) {
        unsafe {
            let cookie = *self.cookie.borrow();
            if cookie != TF_INVALID_COOKIE {
                if let Some(ref mode) = *self.mode.borrow() {
                    if let Ok(source) = mode.cast::<ITfSource>() {
                        let _ = source.UnadviseSink(cookie);
                    }
                }
                *self.cookie.borrow_mut() = TF_INVALID_COOKIE;
            }
        }
    }

    pub fn get_input_mode(&self) -> InputMode {
        self.input_mode.borrow().clone()
    }

    fn notify_input_mode_change(&self) {
        // 简化处理，通知 Alpha 模式
        unsafe {
            if let Some(cb) = &(*self.input_ctx).input_mode_cb {
                cb(InputMode::Alpha);
            }
        }
    }
}

impl ITfCompartmentEventSink_Impl for InputModeHandler_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        log_debug("InputMode OnChange");
        unsafe {
            if !rguid.is_null() && *rguid == GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION {
                self.notify_input_mode_change();
            }
        }
        Ok(())
    }
}

// ============================================================================
// TsInputContextInner
// ============================================================================

pub struct TsInputContextInner {
    pub hwnd: HWND,
    pub thread_mgr: Option<ITfThreadMgr>,
    pub doc_mgr: Option<ITfDocumentMgr>,
    pub empty_doc_mgr: Option<ITfDocumentMgr>,
    pub ctx: Option<ITfContext>,
    pub client_id: u32,
    pub activated: bool,
    pub ui_less: bool,
    pub rect: PreEditRect,
    pub commit_cb: Option<CommitCallback>,
    pub preedit_cb: Option<PreEditCallback>,
    pub candidate_cb: Option<CandidateCallback>,
    pub input_mode_cb: Option<InputModeCallback>,
    pub input_source_cb: Option<InputSourceCallback>,
    pub candidate_config: CandidateConfig,
    pub context_owner: ContextOwner,
    pub composition_handler: CompositionHandler,
    pub input_mode_handler: InputModeHandler,
}

// ============================================================================
// TsInputContext
// ============================================================================

pub struct TsInputContext {
    inner: *mut TsInputContextInner,
}

unsafe impl Send for TsInputContext {}
unsafe impl Sync for TsInputContext {}

impl TsInputContext {
    pub fn new(hwnd: isize, ui_less: bool) -> Option<Box<dyn InputContext>> {
        log_info("Creating TsInputContext");

        if hwnd == 0 {
            log_error("Invalid HWND");
            return None;
        }

        unsafe {
            let hwnd = HWND(hwnd as *mut _);

            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            log_debug("Creating thread manager");
            let thread_mgr: ITfThreadMgr = match CoCreateInstance(&IID_ITfThreadMgr, None, CLSCTX_INPROC_SERVER) {
                Ok(mgr) => mgr,
                Err(e) => {
                    log_error(&format!("Failed to create thread manager: {}", e));
                    return None;
                }
            };

            log_debug("Activating thread manager");
            let thread_mgr_ex: ITfThreadMgrEx = match thread_mgr.cast() {
                Ok(ex) => ex,
                Err(e) => {
                    log_error(&format!("Failed to cast to ITfThreadMgrEx: {}", e));
                    return None;
                }
            };

            let mut client_id = 0u32;
            let hr = if ui_less {
                thread_mgr_ex.ActivateEx(&mut client_id, TF_TMAE_UIELEMENTENABLEDONLY)
            } else {
                client_id = thread_mgr_ex.Activate().unwrap_or(0);
                Ok(())
            };

            if hr.is_err() {
                log_error(&format!("Failed to activate thread manager: {}", hr.unwrap_err()));
                return None;
            }

            log_debug("Creating document managers");
            let doc_mgr = match thread_mgr.CreateDocumentMgr() {
                Ok(mgr) => Some(mgr),
                Err(e) => {
                    log_error(&format!("Failed to create document manager: {}", e));
                    return None;
                }
            };

            let empty_doc_mgr = match thread_mgr.CreateDocumentMgr() {
                Ok(mgr) => Some(mgr),
                Err(e) => {
                    log_error(&format!("Failed to create empty document manager: {}", e));
                    return None;
                }
            };

            log_debug("Deactivating input method initially");
            if let Some(ref empty_doc) = empty_doc_mgr {
                let _ = thread_mgr.AssociateFocus(hwnd, Some(empty_doc));
            }

            log_debug("Creating handlers");
            let inner = Box::new(TsInputContextInner {
                hwnd,
                thread_mgr: Some(thread_mgr.clone()),
                doc_mgr: doc_mgr.clone(),
                empty_doc_mgr: empty_doc_mgr.clone(),
                ctx: None,
                client_id,
                activated: false,
                ui_less,
                rect: PreEditRect::default(),
                commit_cb: None,
                preedit_cb: None,
                candidate_cb: None,
                input_mode_cb: None,
                input_source_cb: None,
                candidate_config: CandidateConfig::default(),
                context_owner: ContextOwner::new(std::ptr::null_mut()),
                composition_handler: CompositionHandler::new(std::ptr::null_mut()),
                input_mode_handler: InputModeHandler::new(std::ptr::null_mut()),
            });

            let inner_ptr = Box::into_raw(inner);

            (*inner_ptr).context_owner.input_ctx = inner_ptr;
            (*inner_ptr).composition_handler.input_ctx = inner_ptr;
            (*inner_ptr).input_mode_handler.input_ctx = inner_ptr;

            log_debug("Creating context");
            if let Some(ref doc_mgr) = doc_mgr {
                let comp_sink: ITfContextOwnerCompositionSink = (&(*inner_ptr).composition_handler).cast().ok()?;
                let mut edit_cookie = 0u32;
                if let Err(e) = doc_mgr.CreateContext(client_id, 0, &comp_sink, &mut (*inner_ptr).ctx, &mut edit_cookie) {
                    log_error(&format!("Failed to create context: {}", e));
                    return None;
                }

                if let Some(ref ctx) = (*inner_ptr).ctx {
                    if let Err(e) = (&(*inner_ptr).composition_handler).initialize(ctx, client_id) {
                        log_warn(&format!("Failed to initialize composition handler: {}", e));
                    }

                    // 注册 ContextOwner sink
                    let owner: ITfContextOwner = (&(*inner_ptr).context_owner).cast().ok()?;
                    if let Ok(source) = ctx.cast::<ITfSource>() {
                        let unknown: IUnknown = owner.cast().ok()?;
                        if let Err(e) = source.AdviseSink(&IID_ITfContextOwner, &unknown) {
                            log_warn(&format!("Failed to advise context owner sink: {}", e));
                        }
                    }
                }

                if let Some(ref ctx) = (*inner_ptr).ctx {
                    if let Err(e) = doc_mgr.Push(ctx) {
                        log_error(&format!("Failed to push context: {}", e));
                        return None;
                    }
                }
            }

            if let Err(e) = (&(*inner_ptr).input_mode_handler).initialize(&thread_mgr) {
                log_warn(&format!("Failed to initialize input mode handler: {}", e));
            }

            log_info("TsInputContext created successfully");

            Some(Box::new(TsInputContext {
                inner: inner_ptr,
            }))
        }
    }
}

impl InputContext for TsInputContext {
    fn get_input_source(&self) -> InputSourceInfo {
        InputSourceInfo::Unsupported
    }

    fn get_input_mode(&self) -> InputMode {
        unsafe {
            (*self.inner).input_mode_handler.get_input_mode()
        }
    }

    fn get_activated(&self) -> bool {
        unsafe { (*self.inner).activated }
    }

    fn set_activated(&mut self, activated: bool) {
        unsafe {
            let inner = &mut *self.inner;
            if activated == inner.activated {
                return;
            }

            log_info(if activated { "Activating TSF" } else { "Deactivating TSF" });
            inner.activated = activated;

            if let Some(ref thread_mgr) = inner.thread_mgr {
                if activated {
                    if let Some(ref doc_mgr) = inner.doc_mgr {
                        let _ = thread_mgr.AssociateFocus(inner.hwnd, Some(doc_mgr));
                    }
                    if let Some(cb) = &inner.input_mode_cb {
                        cb(self.get_input_mode());
                    }
                } else {
                    if let Some(ref empty_doc_mgr) = inner.empty_doc_mgr {
                        let _ = thread_mgr.AssociateFocus(inner.hwnd, Some(empty_doc_mgr));
                    }
                }
            }
        }
    }

    fn set_preedit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            (*self.inner).rect = PreEditRect { x, y, width, height };
        }
    }

    fn set_commit_callback(&mut self, callback: CommitCallback) {
        unsafe {
            (*self.inner).commit_cb = Some(callback);
        }
    }

    fn set_preedit_callback(&mut self, callback: PreEditCallback) {
        unsafe {
            (*self.inner).preedit_cb = Some(callback);
        }
    }

    fn set_candidate_callback(&mut self, callback: CandidateCallback) {
        unsafe {
            (*self.inner).candidate_cb = Some(callback);
        }
    }

    fn set_input_source_callback(&mut self, callback: InputSourceCallback) {
        unsafe {
            (*self.inner).input_source_cb = Some(callback);
        }
    }

    fn set_input_mode_callback(&mut self, callback: InputModeCallback) {
        unsafe {
            (*self.inner).input_mode_cb = Some(callback);
        }
    }

    fn get_candidate_config(&self) -> CandidateConfig {
        unsafe { (*self.inner).candidate_config.clone() }
    }

    fn set_candidate_config(&mut self, config: CandidateConfig) {
        unsafe {
            (*self.inner).candidate_config = config;
        }
    }
}

impl Drop for TsInputContext {
    fn drop(&mut self) {
        log_info("Dropping TsInputContext");

        unsafe {
            let inner = Box::from_raw(self.inner);

            if let Some(ref ctx) = inner.ctx {
                inner.context_owner.unadvise(ctx);
                inner.composition_handler.unadvise_sinks(ctx);
            }
            inner.input_mode_handler.unadvise_sink();

            if inner.activated {
                if let Some(ref thread_mgr) = inner.thread_mgr {
                    if let Some(ref empty_doc_mgr) = inner.empty_doc_mgr {
                        let _ = thread_mgr.AssociateFocus(inner.hwnd, Some(empty_doc_mgr));
                    }
                }
            }

            if let Some(ref doc_mgr) = inner.doc_mgr {
                let _ = doc_mgr.Pop(TF_POPF_ALL);
            }

            if inner.client_id != 0 {
                if let Some(ref thread_mgr) = inner.thread_mgr {
                    let _ = thread_mgr.Deactivate();
                }
            }
        }

        log_info("TsInputContext dropped");
    }
}
