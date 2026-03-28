//! JNI bridge for IngameIME Rust core.
//! Exposes RustImeLibrary native methods to Java.

#![allow(non_snake_case)]

use jni::objects::{JClass, JObject, JValue, JObjectArray, JString, GlobalRef};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use std::num::NonZeroIsize;
use std::sync::OnceLock;
use jni::JNIEnv;

#[cfg(windows)]
use crate::interface::imm32::Imm32InputContext;

use crate::interface::lib::{
    CandidateEvent, InputMode, PreEditEvent,
};

// ============================================================================
// Initialize logger on first load
// ============================================================================

static LOGGER_INIT: OnceLock<()> = OnceLock::new();

fn init_logger() {
    LOGGER_INIT.get_or_init(|| {
        // Simple logger that outputs to stdout
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    });
}

// ============================================================================
// Global JavaVM — stored on first context creation, used by all callbacks
// ============================================================================

static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();

fn get_java_vm() -> Option<&'static jni::JavaVM> {
    JAVA_VM.get()
}

fn init_java_vm(env: &JNIEnv) {
    if JAVA_VM.get().is_none() {
        if let Ok(vm) = env.get_java_vm() {
            let _ = JAVA_VM.set(vm);
        }
    }
}

// ============================================================================
// ImeContext — thin wrapper that owns the dyn InputContext
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
    init_logger();
    init_java_vm(&env);

    let hwnd_nz = match NonZeroIsize::new(hwnd as isize) {
        Some(nz) => nz,
        None => {
            log::error!("Invalid HWND (0) passed to create_input_context");
            return 0;
        }
    };

    let is_ui_less: bool = ui_less != JNI_FALSE;

    let ctx: Option<Box<dyn crate::interface::lib::InputContext>> = match api {
        1 => {
            log::info!("Creating IMM32 InputContext (ui_less={})", is_ui_less);
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
            log::error!("Unknown API type: {}", api);
            None
        }
    };

    match ctx {
        Some(c) => {
            let wrapper = Box::new(ImeContext::new(c));
            let ptr = Box::into_raw(wrapper);
            ptr as jlong
        },
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
// Callback registration
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_rust_1ime_1library_1set_1commit_1callback(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    callback: JObject,
) {
    if callback.is_null() { return; }
    if ptr == 0 { log::warn!("set_commit_callback: null ptr"); return; }

    let Ok(global_ref) = env.new_global_ref(callback) else { return };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper.ctx.set_commit_callback(Box::new(move |text: String| {
            let Some(vm) = get_java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread_permanently().and_then(|_| vm.get_env()) else { return };
            let Ok(jstr) = env.new_string(&text) else { return };
            let _ = env.call_method(&global_ref, "onCommit", "(Ljava/lang/String;)V", &[JValue::Object(&jstr)]);
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
    if callback.is_null() { return; }
    if ptr == 0 { log::warn!("set_preedit_callback: null ptr"); return; }

    let Ok(global_ref) = env.new_global_ref(callback) else { return };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper.ctx.set_preedit_callback(Box::new(move |event: PreEditEvent| {
            let Some(vm) = get_java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread_permanently().and_then(|_| vm.get_env()) else { return };
            match &event {
                PreEditEvent::Begin => {
                    let _ = env.call_method(&global_ref, "onPreEdit", "(ILjava/lang/String;I)V",
                        &[JValue::Int(0), JValue::Object(&JObject::null()), JValue::Int(-1)]);
                }
                PreEditEvent::Update(preedit) => {
                    let Ok(jstr) = env.new_string(&preedit.text) else { return };
                    let _ = env.call_method(&global_ref, "onPreEdit", "(ILjava/lang/String;I)V",
                        &[JValue::Int(1), JValue::Object(&jstr), JValue::Int(preedit.cursor as jint)]);
                }
                PreEditEvent::End => {
                    let _ = env.call_method(&global_ref, "onPreEdit", "(ILjava/lang/String;I)V",
                        &[JValue::Int(2), JValue::Object(&JObject::null()), JValue::Int(-1)]);
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
    if callback.is_null() { return; }
    if ptr == 0 { log::warn!("set_candidate_callback: null ptr"); return; }

    let Ok(global_ref) = env.new_global_ref(callback) else { return };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper.ctx.set_candidate_callback(Box::new(move |event: CandidateEvent| {
            let Some(vm) = get_java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread_permanently().and_then(|_| vm.get_env()) else { return };
            match &event {
                CandidateEvent::Begin => {
                    let _ = env.call_method(&global_ref, "onCandidateList", "(I[Ljava/lang/String;I)V",
                        &[JValue::Int(0), JValue::Object(&JObject::null()), JValue::Int(-1)]);
                }
                CandidateEvent::Update(candidate) => {
                    let Ok(arr) = env.new_object_array(candidate.candidates.len() as jint, "java/lang/String", JObject::null()) else { return };
                    for (i, s) in candidate.candidates.iter().enumerate() {
                        let Ok(jstr) = env.new_string(s) else { continue };
                        let _ = env.set_object_array_element(&arr, i as jint, jstr);
                    }
                    let _ = env.call_method(&global_ref, "onCandidateList", "(I[Ljava/lang/String;I)V",
                        &[JValue::Int(1), JValue::Object(&arr), JValue::Int(candidate.selected as jint)]);
                }
                CandidateEvent::End => {
                    let _ = env.call_method(&global_ref, "onCandidateList", "(I[Ljava/lang/String;I)V",
                        &[JValue::Int(2), JValue::Object(&JObject::null()), JValue::Int(-1)]);
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
    if callback.is_null() { return; }
    if ptr == 0 { log::warn!("set_input_mode_callback: null ptr"); return; }

    let Ok(global_ref) = env.new_global_ref(callback) else { return };

    unsafe {
        let wrapper = &mut *(ptr as *mut ImeContext);
        wrapper.ctx.set_input_mode_callback(Box::new(move |mode: InputMode| {
            let Some(vm) = get_java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread_permanently().and_then(|_| vm.get_env()) else { return };
            let mode_int: jint = match mode {
                InputMode::Alpha => 0,
                InputMode::Native => 1,
                InputMode::Unsupported => 2,
            };
            let _ = env.call_method(&global_ref, "onInputModeChanged", "(I)V", &[JValue::Int(mode_int)]);
        }));
    }
}
