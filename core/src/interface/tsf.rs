#![allow(dead_code)]
#![allow(unused_variables)]

use crate::interface::lib::{
    CandidateCallback, CandidateConfig, CommitCallback, InputContext,
    InputMode, InputModeCallback, InputSourceCallback, InputSourceInfo, PreEditCallback,
};

fn log_info(msg: &str) {
    crate::interface::jni_api::log_info(msg);
}

pub struct TsInputContext {
    activated: bool,
    rect: PreEditRect,
    commit_cb: Option<CommitCallback>,
    preedit_cb: Option<PreEditCallback>,
    candidate_cb: Option<CandidateCallback>,
    input_mode_cb: Option<InputModeCallback>,
    input_source_cb: Option<InputSourceCallback>,
    candidate_config: CandidateConfig,
    input_mode: InputMode,
}

#[derive(Clone, Copy, Default)]
pub struct PreEditRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

unsafe impl Send for TsInputContext {}
unsafe impl Sync for TsInputContext {}

impl TsInputContext {
    pub fn new(hwnd: isize, ui_less: bool) -> Option<Box<dyn InputContext>> {
        log_info("Creating TsInputContext (simplified)");
        log_info("注意：当前为简化实现，建议使用 IMM32 (api=1) 获取完整功能");
        log_info("完整 TSF 实现需要参考 IngameIME_Win32 项目");

        Some(Box::new(TsInputContext {
            activated: false,
            rect: PreEditRect::default(),
            commit_cb: None,
            preedit_cb: None,
            candidate_cb: None,
            input_mode_cb: None,
            input_source_cb: None,
            candidate_config: CandidateConfig::default(),
            input_mode: InputMode::Alpha,
        }))
    }
}

impl InputContext for TsInputContext {
    fn get_input_source(&self) -> InputSourceInfo {
        InputSourceInfo::Unsupported
    }

    fn get_input_mode(&self) -> InputMode {
        self.input_mode
    }

    fn get_activated(&self) -> bool {
        self.activated
    }

    fn set_activated(&mut self, activated: bool) {
        if activated == self.activated {
            return;
        }
        log_info(if activated { "Activating TSF (stub)" } else { "Deactivating TSF (stub)" });
        self.activated = activated;
    }

    fn set_preedit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.rect = PreEditRect { x, y, width, height };
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
