//! JNI bridge for IngameIME Rust core.
//! Exposes RustImeLibrary native methods to Java.

#![allow(non_snake_case)]

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use std::num::NonZeroIsize;
use std::sync::{Mutex, OnceLock};

use crate::interface::lib::{CandidateConfig, CandidateEvent, InputMode, PreEditEvent};

// ============================================================================
// Global JavaVM and Logger reference �?stored on first context creation
// ============================================================================

static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();
static JAVA_LOGGER: OnceLock<Mutex<Option<GlobalRef>>> = OnceLock::new();
static DEBUG_LOGGING: OnceLock<Mutex<bool>> = OnceLock::new();

fn init_java_vm(env: &JNIEnv) {
    if JAVA_VM.get().is_none() {
        if let Ok(vm) = env.get_java_vm() {
            let _ = JAVA_VM.set(vm);
        }
    }
}

fn get_java_vm() -> Option<&'static jni::JavaVM> {
    JAVA_VM.get()
}

/// Initialize Java logger reference
fn init_java_logger(env: &mut JNIEnv) {
    if JAVA_LOGGER.get().is_none() {
        if let Ok(logger_class) = env.find_class("org/apache/logging/log4j/LogManager") {
            if let Ok(logger_obj) = env.call_static_method(
                &logger_class,
                "getLogger",
                "(Ljava/lang/String;)Lorg/apache/logging/log4j/Logger;",
                &[JValue::Object(
                    &env.new_string("IngameIME-Rust").unwrap().into(),
                )],
            ) {
                if let Ok(logger_global) = env.new_global_ref(logger_obj.l().unwrap()) {
                    let _ = JAVA_LOGGER.set(Mutex::new(Some(logger_global)));
                }
            }
        }
    }
}

/// Log a message to Java logger
fn log_to_java(level: &str, message: &str) {
    if let Some(vm) = get_java_vm() {
        if let Ok(mut env) = vm.attach_current_thread_permanently() {
            if let Some(logger_guard) = JAVA_LOGGER.get() {
                if let Ok(logger_opt) = logger_guard.lock() {
                    if let Some(logger) = logger_opt.as_ref() {
                        let Ok(jlevel) = env.new_string(level) else {
                            return;
                        };
                        let Ok(jmsg) = env.new_string(message) else {
                            return;
                        };
                        let _ = env.call_method(
                            logger,
                            "log",
                            "(Lorg/apache/logging/log4j/Level;Ljava/lang/Object;)V",
                            &[
                                JValue::Object(&env.new_string(level).unwrap().into()),
                                JValue::Object(&jmsg.into()),
                            ],
                        );
                    }
                }
            }
        }
    }
}

/// Simpler logging using println that gets captured by Forge
pub fn log_info(message: &str) {
    println!("[IngameIME-Rust] {}", message);
}

pub fn log_debug(message: &str) {
    // Always output debug logs for candidate info
    // Check DEBUG_LOGGING but default to true if not set
    let should_log = if let Some(debug_guard) = DEBUG_LOGGING.get() {
        if let Ok(debug) = debug_guard.lock() {
            *debug
        } else {
            true // Default to true if lock fails
        }
    } else {
        true // Default to true if not initialized
    };

    if should_log {
        println!("[IngameIME-Rust-DEBUG] {}", message);
    }
}

/// Enable or disable debug logging (called from Java)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1debug_1logging(
    _env: JNIEnv,
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
// ImeContext �?thin wrapper that owns the dyn InputContext
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
// Context lifecycle
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1create_1input_1context_1win32(
    env: JNIEnv,
    _class: JClass,
    hwnd: jlong,
    api: jint,
    ui_less: jboolean,
) -> jlong {
    init_java_vm(&env);

    let hwnd_nz = match NonZeroIsize::new(hwnd as isize) {
        Some(nz) => nz,
        None => {
            log_info("Invalid HWND (0) passed to create_input_context");
            return 0;
        }
    };

    let is_ui_less: bool = ui_less != JNI_FALSE;

    let ctx: Option<Box<dyn crate::interface::lib::InputContext>> = match api {
        1 => {
            log_info(&format!(
                "Creating IMM32 InputContext (ui_less={})",
                is_ui_less
            ));
            #[cfg(windows)]
            {
                crate::interface::imm32::Imm32InputContext::new(hwnd_nz, is_ui_less)
            }
            #[cfg(not(windows))]
            {
                None
            }
        }
        _ => {
            log_info(&format!("ERROR: Unknown API type: {}", api));
            None
        }
    };

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
    _env: JNIEnv,
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
    _env: JNIEnv,
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
    _env: JNIEnv,
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
    _env: JNIEnv,
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
    _env: JNIEnv,
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
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let version = env!("CARGO_PKG_VERSION");
    match env.new_string(version) {
        Ok(s) => s.into_raw(),
        Err(_) => JObject::null().into_raw(),
    }
}

// ============================================================================
// Candidate configuration
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1max_1candidates(
    _env: JNIEnv,
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
    _env: JNIEnv,
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
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() {
        return;
    }
    if ptr == 0 {
        log_info("WARN: set_commit_callback: null ptr");
        return;
    }

    let Ok(global_ref) = env.new_global_ref(callback) else {
        return;
    };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper
            .ctx
            .set_commit_callback(Box::new(move |text: String| {
                let Some(vm) = get_java_vm() else { return };
                let Ok(mut env) = vm
                    .attach_current_thread_permanently()
                    .and_then(|_| vm.get_env())
                else {
                    return;
                };
                let Ok(jstr) = env.new_string(&text) else {
                    return;
                };
                let _ = env.call_method(
                    &global_ref,
                    "onCommit",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&jstr)],
                );
            }));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1pre_1edit_1callback(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() {
        return;
    }
    if ptr == 0 {
        log_info("WARN: set_preedit_callback: null ptr");
        return;
    }

    let Ok(global_ref) = env.new_global_ref(callback) else {
        return;
    };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper
            .ctx
            .set_preedit_callback(Box::new(move |event: PreEditEvent| {
                let Some(vm) = get_java_vm() else { return };
                let Ok(mut env) = vm
                    .attach_current_thread_permanently()
                    .and_then(|_| vm.get_env())
                else {
                    return;
                };
                match &event {
                    PreEditEvent::Begin => {
                        let _ = env.call_method(
                            &global_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[
                                JValue::Int(0),
                                JValue::Object(&JObject::null()),
                                JValue::Int(-1),
                            ],
                        );
                    }
                    PreEditEvent::Update(preedit) => {
                        let Ok(jstr) = env.new_string(&preedit.text) else {
                            return;
                        };
                        let _ = env.call_method(
                            &global_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[
                                JValue::Int(1),
                                JValue::Object(&jstr),
                                JValue::Int(preedit.cursor as jint),
                            ],
                        );
                    }
                    PreEditEvent::End => {
                        let _ = env.call_method(
                            &global_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[
                                JValue::Int(2),
                                JValue::Object(&JObject::null()),
                                JValue::Int(-1),
                            ],
                        );
                    }
                }
            }));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1candidate_1list_1callback(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() {
        return;
    }
    if ptr == 0 {
        log_info("WARN: set_candidate_callback: null ptr");
        return;
    }

    let Ok(global_ref) = env.new_global_ref(callback) else {
        return;
    };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper
            .ctx
            .set_candidate_callback(Box::new(move |event: CandidateEvent| {
                let Some(vm) = get_java_vm() else { return };
                let Ok(mut env) = vm
                    .attach_current_thread_permanently()
                    .and_then(|_| vm.get_env())
                else {
                    return;
                };
                match &event {
                    CandidateEvent::Begin => {
                        let _ = env.call_method(
                            &global_ref,
                            "onCandidateList",
                            "(I[Ljava/lang/String;I)V",
                            &[
                                JValue::Int(0),
                                JValue::Object(&JObject::null()),
                                JValue::Int(-1),
                            ],
                        );
                    }
                    CandidateEvent::Update(candidate) => {
                        let Ok(arr) = env.new_object_array(
                            candidate.candidates.len() as jint,
                            "java/lang/String",
                            JObject::null(),
                        ) else {
                            return;
                        };
                        for (i, s) in candidate.candidates.iter().enumerate() {
                            let Ok(jstr) = env.new_string(s) else {
                                continue;
                            };
                            let _ = env.set_object_array_element(&arr, i as jint, jstr);
                        }
                        let _ = env.call_method(
                            &global_ref,
                            "onCandidateList",
                            "(I[Ljava/lang/String;I)V",
                            &[
                                JValue::Int(1),
                                JValue::Object(&arr),
                                JValue::Int(candidate.selected as jint),
                            ],
                        );
                    }
                    CandidateEvent::End => {
                        let _ = env.call_method(
                            &global_ref,
                            "onCandidateList",
                            "(I[Ljava/lang/String;I)V",
                            &[
                                JValue::Int(2),
                                JValue::Object(&JObject::null()),
                                JValue::Int(-1),
                            ],
                        );
                    }
                }
            }));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1input_1mode_1callback(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() {
        return;
    }
    if ptr == 0 {
        log_info("WARN: set_input_mode_callback: null ptr");
        return;
    }

    let Ok(global_ref) = env.new_global_ref(callback) else {
        return;
    };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper
            .ctx
            .set_input_mode_callback(Box::new(move |mode: InputMode| {
                let Some(vm) = get_java_vm() else { return };
                let Ok(mut env) = vm
                    .attach_current_thread_permanently()
                    .and_then(|_| vm.get_env())
                else {
                    return;
                };
                let mode_int: jint = match mode {
                    InputMode::Alpha => 0,
                    InputMode::Native => 1,
                    InputMode::Unsupported => 2,
                };
                let _ = env.call_method(
                    &global_ref,
                    "onInputModeChanged",
                    "(I)V",
                    &[JValue::Int(mode_int)],
                );
            }));
    }
}
