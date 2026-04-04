//! JNI bridge for IngameIME Rust core.
//! Exposes RustImeLibrary native methods to Java.

#![allow(non_snake_case)]

use jni::objects::{Global, JClass, JObject, JValue};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE, JavaVM as SysJavaVM};
use jni::{EnvUnowned, JavaVM, jni_str, jni_sig};
use std::num::NonZeroIsize;
use std::sync::{Mutex, OnceLock};

use crate::interface::lib::{CandidateConfig, CandidateEvent, InputMode, PreEditEvent};

// ============================================================================
// Global JavaVM and Logger reference - stored on first context creation
// ============================================================================

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static JAVA_LOGGER: OnceLock<Mutex<Option<Global<JObject<'static>>>>> = OnceLock::new();
static DEBUG_LOGGING: OnceLock<Mutex<bool>> = OnceLock::new();

/// JNI_OnLoad is called when the library is loaded
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: *mut SysJavaVM, _reserved: *mut std::ffi::c_void) -> jint {
    unsafe {
        let java_vm = JavaVM::from_raw(vm);
        let _ = JAVA_VM.set(java_vm);
        log_info("JNI_OnLoad called, JavaVM stored");
    }
    jni_sys::JNI_VERSION_1_8 as jint
}

fn get_java_vm() -> Option<&'static JavaVM> {
    JAVA_VM.get()
}

/// Simpler logging using println that gets captured by Forge
pub fn log_info(message: &str) {
    println!("[IngameIME-Rust] {}", message);
}

pub fn log_debug(message: &str) {
    let should_log = if let Some(debug_guard) = DEBUG_LOGGING.get() {
        if let Ok(debug) = debug_guard.lock() {
            *debug
        } else {
            true
        }
    } else {
        true
    };

    if should_log {
        println!("[IngameIME-Rust-DEBUG] {}", message);
    }
}

// ============================================================================
// ImeContext - thin wrapper that owns the dyn InputContext
// ============================================================================

pub struct ImeContext {
    pub ctx: Box<dyn crate::interface::lib::InputContext>,
}

impl ImeContext {
    pub fn new(ctx: Box<dyn crate::interface::lib::InputContext>) -> Self {
        Self { ctx }
    }
}

// ============================================================================
// Enable or disable debug logging
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1debug_1logging(
    _env: EnvUnowned,
    _class: JClass,
    enabled: jboolean,
) {
    let is_enabled = enabled != JNI_FALSE;
    let _ = DEBUG_LOGGING.set(Mutex::new(is_enabled));
    if is_enabled {
        log_info("Debug logging enabled");
    }
}

// ============================================================================
// Context lifecycle
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1create_1input_1context_1win32(
    _env: EnvUnowned,
    _class: JClass,
    hwnd: jlong,
    api: jint,
    ui_less: jboolean,
) -> jlong {
    let hwnd_nz = match NonZeroIsize::new(hwnd as isize) {
        Some(nz) => nz,
        None => {
            log_info("Invalid HWND (0) passed to create_input_context");
            return 0;
        }
    };

    let is_ui_less: bool = ui_less != JNI_FALSE;

    // Try requested API first, then fallback to IMM32 if failed
    let mut ctx: Option<Box<dyn crate::interface::lib::InputContext>> = None;
    let mut tried_imm32 = false;

    if api == 0 {
        // Try TSF first
        log_info(&format!(
            "Creating TSF InputContext (ui_less={})",
            is_ui_less
        ));
        #[cfg(windows)]
        {
            ctx = crate::interface::tsf::TsInputContext::new(hwnd as isize, is_ui_less);
        }

        // If TSF failed, fallback to IMM32
        if ctx.is_none() {
            log_info("TSF initialization failed, falling back to IMM32...");
            tried_imm32 = true;
        }
    }

    // Use IMM32 if requested or if TSF failed
    if ctx.is_none() {
        if tried_imm32 || api == 1 {
            log_info(&format!(
                "Creating IMM32 InputContext (ui_less={})",
                is_ui_less
            ));
            #[cfg(windows)]
            {
                ctx = crate::interface::imm32::Imm32InputContext::new(hwnd_nz, is_ui_less);
            }
            #[cfg(not(windows))]
            {
                ctx = None;
            }
        }
    }

    if ctx.is_none() {
        log_info(&format!("ERROR: Failed to create InputContext for API {}", api));
    }

    match ctx {
        Some(c) => {
            let wrapper = Box::new(ImeContext::new(c));
            let ptr = Box::into_raw(wrapper);
            ptr as jlong
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1destroy_1input_1context(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe {
            let _ = Box::from_raw(ptr as *mut ImeContext);
        }
    }
}

// ============================================================================
// Activation
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1input_1context_1activated(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    activated: jboolean,
) {
    if ptr != 0 {
        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper.ctx.set_activated(activated != JNI_FALSE);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1is_1input_1context_1activated(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
) -> jboolean {
    if ptr != 0 {
        unsafe {
            let wrapper = &*(ptr as *mut ImeContext);
            if wrapper.ctx.get_activated() {
                JNI_TRUE
            } else {
                JNI_FALSE
            }
        }
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1get_1input_1mode(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
) -> jint {
    if ptr != 0 {
        unsafe {
            let wrapper = &*(ptr as *mut ImeContext);
            match wrapper.ctx.get_input_mode() {
                InputMode::Alpha => 0,
                InputMode::Native => 1,
                InputMode::Unsupported => 2,
            }
        }
    } else {
        2
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1pre_1edit_1rect(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) {
    if ptr != 0 {
        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper.ctx.set_preedit_rect(x, y, width, height);
        }
    }
}

// ============================================================================
// Library version
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1get_1version(
    mut env: EnvUnowned,
    _class: JClass,
) -> jstring {
    let mut result: jstring = JObject::null().into_raw();
    env.with_env(|env| {
        let version = env!("CARGO_PKG_VERSION");
        if let Ok(s) = env.new_string(version) {
            result = s.into_raw();
        }
        Ok::<(), jni::errors::Error>(())
    }).resolve::<jni::errors::LogErrorAndDefault>();
    result
}

// ============================================================================
// Candidate configuration
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1max_1candidates(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    max_candidates: jint,
) {
    if ptr == 0 {
        log_info("set_max_candidates: null ptr");
        return;
    }
    let max = if max_candidates > 0 {
        max_candidates as usize
    } else {
        9 // default
    };
    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper.ctx.set_candidate_config(CandidateConfig {
            max_candidates: max,
        });
        log_info(&format!("set_max_candidates: max={}", max));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1get_1max_1candidates(
    _env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
) -> jint {
    if ptr == 0 {
        log_info("get_max_candidates: null ptr");
        return 0;
    }
    unsafe {
        let wrapper = &*(ptr as *mut ImeContext);
        wrapper.ctx.get_candidate_config().max_candidates as jint
    }
}

// ============================================================================
// Callback registration
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1commit_1callback(
    mut env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() || ptr == 0 {
        return;
    }

    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(global_ref) = env.new_global_ref(&callback) else {
            return Ok(());
        };

        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper
                .ctx
                .set_commit_callback(Box::new(move |text: String| {
                    let Some(vm) = get_java_vm() else {
                        log_debug("Java VM not available");
                        return;
                    };
                    let _: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
                        if let Ok(jtext) = env.new_string(&text) {
                            let _ = env.call_method(
                                &global_ref,
                                jni_str!("onCommit"),
                                jni_sig!("(Ljava/lang/String;)V"),
                                &[JValue::Object(&jtext)],
                            );
                        }
                        Ok(())
                    });
                }));
        }
        Ok(())
    }).resolve::<jni::errors::LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1pre_1edit_1callback(
    mut env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() || ptr == 0 {
        return;
    }

    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(global_ref) = env.new_global_ref(&callback) else {
            return Ok(());
        };

        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper
                .ctx
                .set_preedit_callback(Box::new(move |event: PreEditEvent| {
                    let Some(vm) = get_java_vm() else {
                        log_debug("Java VM not available");
                        return;
                    };
                    let _: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
                        match &event {
                            PreEditEvent::Begin => {
                                let _ = env.call_method(
                                    &global_ref,
                                    jni_str!("onPreEdit"),
                                    jni_sig!("(ILjava/lang/String;I)V"),
                                    &[
                                        JValue::Int(0), // Begin
                                        JValue::Object(&JObject::null()),
                                        JValue::Int(-1),
                                    ],
                                );
                            }
                            PreEditEvent::Update(preedit) => {
                                if let Ok(jtext) = env.new_string(&preedit.text) {
                                    let _ = env.call_method(
                                        &global_ref,
                                        jni_str!("onPreEdit"),
                                        jni_sig!("(ILjava/lang/String;I)V"),
                                        &[
                                            JValue::Int(1), // Update
                                            JValue::Object(&jtext),
                                            JValue::Int(preedit.cursor as jint),
                                        ],
                                    );
                                }
                            }
                            PreEditEvent::End => {
                                let _ = env.call_method(
                                    &global_ref,
                                    jni_str!("onPreEdit"),
                                    jni_sig!("(ILjava/lang/String;I)V"),
                                    &[
                                        JValue::Int(2), // End
                                        JValue::Object(&JObject::null()),
                                        JValue::Int(-1),
                                    ],
                                );
                            }
                        }
                        Ok(())
                    });
                }));
        }
        Ok(())
    }).resolve::<jni::errors::LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1candidate_1list_1callback(
    mut env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() || ptr == 0 {
        return;
    }

    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(global_ref) = env.new_global_ref(&callback) else {
            return Ok(());
        };

        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper
                .ctx
                .set_candidate_callback(Box::new(move |event: CandidateEvent| {
                    let Some(vm) = get_java_vm() else {
                        log_debug("Java VM not available");
                        return;
                    };
                    let _: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
                        match &event {
                            CandidateEvent::Begin => {
                                let _ = env.call_method(
                                    &global_ref,
                                    jni_str!("onCandidateList"),
                                    jni_sig!("(I[Ljava/lang/String;I)V"),
                                    &[
                                        JValue::Int(0), // Begin
                                        JValue::Object(&JObject::null()),
                                        JValue::Int(-1),
                                    ],
                                );
                            }
                            CandidateEvent::Update(candidate) => {
                                let arr = env.new_object_array(
                                    candidate.candidates.len() as jint,
                                    jni_str!("java/lang/String"),
                                    JObject::null(),
                                );
                                if let Ok(arr) = arr {
                                    for (i, s) in candidate.candidates.iter().enumerate() {
                                        if let Ok(jstr) = env.new_string(s) {
                                            let _ = arr.set_element(env, i as usize, &jstr);
                                        }
                                    }
                                    let _ = env.call_method(
                                        &global_ref,
                                        jni_str!("onCandidateList"),
                                        jni_sig!("(I[Ljava/lang/String;I)V"),
                                        &[
                                            JValue::Int(1), // Update
                                            JValue::Object(&arr),
                                            JValue::Int(candidate.selected as jint),
                                        ],
                                    );
                                }
                            }
                            CandidateEvent::End => {
                                let _ = env.call_method(
                                    &global_ref,
                                    jni_str!("onCandidateList"),
                                    jni_sig!("(I[Ljava/lang/String;I)V"),
                                    &[
                                        JValue::Int(2), // End
                                        JValue::Object(&JObject::null()),
                                        JValue::Int(-1),
                                    ],
                                );
                            }
                        }
                        Ok(())
                    });
                }));
        }
        Ok(())
    }).resolve::<jni::errors::LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1input_1mode_1callback(
    mut env: EnvUnowned,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() || ptr == 0 {
        return;
    }

    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let Ok(global_ref) = env.new_global_ref(&callback) else {
            return Ok(());
        };

        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper
                .ctx
                .set_input_mode_callback(Box::new(move |mode: InputMode| {
                    let Some(vm) = get_java_vm() else {
                        log_debug("Java VM not available");
                        return;
                    };
                    let _: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
                        let mode_int: jint = match mode {
                            InputMode::Alpha => 0,
                            InputMode::Native => 1,
                            InputMode::Unsupported => 2,
                        };
                        let _ = env.call_method(
                            &global_ref,
                            jni_str!("onInputModeChanged"),
                            jni_sig!("(I)V"),
                            &[JValue::Int(mode_int)],
                        );
                        Ok(())
                    });
                }));
        }
        Ok(())
    }).resolve::<jni::errors::LogErrorAndDefault>();
}


