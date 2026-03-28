//! Simple JNI API for IngameIME Rust core
//! No SWIG dependencies - clean and straightforward

use jni::objects::{JClass, JObject, JString, GlobalRef, JValue, JObjectArray};
use jni::sys::{jboolean, jint, jlong, JNI_TRUE, JNI_FALSE};
use jni::JNIEnv;
use std::num::NonZeroIsize;
use std::sync::OnceLock;

#[cfg(windows)]
use crate::interface::imm32::Imm32InputContext;
use crate::interface::lib::{InputContext, InputMode, PreEditEvent, CandidateEvent};

// ============================================================================
// Global state
// ============================================================================

static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();

fn get_java_vm() -> Option<&'static jni::JavaVM> {
    JAVA_VM.get()
}

// ============================================================================
// Context wrapper
// ============================================================================

pub struct ImeContext {
    ctx: Box<dyn InputContext>,
    commit_callback: Option<GlobalRef>,
    preedit_callback: Option<GlobalRef>,
    candidate_callback: Option<GlobalRef>,
    input_mode_callback: Option<GlobalRef>,
}

impl ImeContext {
    pub fn new(ctx: Box<dyn InputContext>) -> Self {
        Self {
            ctx,
            commit_callback: None,
            preedit_callback: None,
            candidate_callback: None,
            input_mode_callback: None,
        }
    }
    
    /// Setup a specific callback type
    fn setup_commit_callback(&mut self, cb_ref: GlobalRef) {
        let Some(vm) = get_java_vm() else { return };

        self.ctx.set_commit_callback(Box::new(move |text: String| {
            if let Ok(mut env) = vm.attach_current_thread() {
                let jtext: JString = env.new_string(&text).unwrap_or_else(|_| JObject::null().into());
                let _ = env.call_method(
                    &cb_ref,
                    "onCommit",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&jtext)],
                );
            }
        }));
    }

    fn setup_preedit_callback(&mut self, cb_ref: GlobalRef) {
        let Some(vm) = get_java_vm() else { return };

        self.ctx.set_preedit_callback(Box::new(move |event: PreEditEvent| {
            if let Ok(mut env) = vm.attach_current_thread() {
                match event {
                    PreEditEvent::Begin => {
                        let _ = env.call_method(
                            &cb_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[JValue::Int(0), JValue::Object(&JObject::null()), JValue::Int(0)],
                        );
                    }
                    PreEditEvent::Update(preedit) => {
                        let jtext: JString = env.new_string(&preedit.text).unwrap_or_else(|_| JObject::null().into());
                        let _ = env.call_method(
                            &cb_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[JValue::Int(1), JValue::Object(&jtext), JValue::Int(preedit.cursor as jint)],
                        );
                    }
                    PreEditEvent::End => {
                        let _ = env.call_method(
                            &cb_ref,
                            "onPreEdit",
                            "(ILjava/lang/String;I)V",
                            &[JValue::Int(2), JValue::Object(&JObject::null()), JValue::Int(0)],
                        );
                    }
                }
            }
        }));
    }

    fn setup_candidate_callback(&mut self, cb_ref: GlobalRef) {
        let Some(vm) = get_java_vm() else { return };
        let cb_global = cb_ref;
        
        self.ctx.set_candidate_callback(Box::new(move |event: CandidateEvent| {
            if let Ok(mut env) = vm.attach_current_thread() {
                match event {
                    CandidateEvent::Begin => {
                        let _ = env.call_method(
                            &cb_global,
                            "onCandidateList",
                            "(I[Ljava/lang/String;I)V",
                            &[JValue::Int(0), JValue::Object(&JObject::null()), JValue::Int(0)],
                        );
                    }
                    CandidateEvent::Update(candidate) => {
                        let arr: JObjectArray = env.new_object_array(
                            candidate.candidates.len() as jint,
                            "java/lang/String",
                            JObject::null(),
                        ).unwrap_or_else(|_| JObject::null().into());

                        for (i, text) in candidate.candidates.iter().enumerate() {
                            let jtext: JString = env.new_string(text).unwrap_or_else(|_| JObject::null().into());
                            let _ = env.set_object_array_element(&arr, i as jint, &jtext);
                        }

                        let _ = env.call_method(
                            &cb_global,
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
                            &cb_global,
                            "onCandidateList",
                            "(I[Ljava/lang/String;I)V",
                            &[JValue::Int(2), JValue::Object(&JObject::null()), JValue::Int(0)],
                        );
                    }
                }
            }
        }));
    }
    
    fn setup_input_mode_callback(&mut self, cb_ref: GlobalRef) {
        let Some(vm) = get_java_vm() else { return };

        self.ctx.set_input_mode_callback(Box::new(move |mode: InputMode| {
            if let Ok(mut env) = vm.attach_current_thread() {
                let mode_int = match mode {
                    InputMode::Alpha => 0,
                    InputMode::Native => 1,
                    InputMode::Unsupported => 2,
                };
                let _ = env.call_method(
                    &cb_ref,
                    "onInputModeChanged",
                    "(I)V",
                    &[JValue::Int(mode_int)],
                );
            }
        }));
    }
}

// ============================================================================
// JNI Functions - RustImeLibrary
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_getVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.new_string(crate::interface::lib::VERSION)
        .unwrap_or_else(|_| JObject::null().into())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_createInputContextWin32__JIZ(
    env: JNIEnv,
    _class: JClass,
    hwnd: jlong,
    api: jint,
    ui_less: jboolean,
) -> jlong {
    #[cfg(windows)]
    {
        log::info!("Creating InputContext: hwnd=0x{:x}, api={}, ui_less={}", hwnd, api, ui_less != 0);

        if let Ok(vm) = env.get_java_vm() {
            let _ = JAVA_VM.set(vm);
            log::info!("JavaVM initialized");
        }

        let hwnd_nonzero = match NonZeroIsize::new(hwnd as isize) {
            Some(h) => h,
            None => {
                log::error!("Invalid HWND: 0");
                return 0;
            }
        };

        if let Some(ctx) = Imm32InputContext::new(hwnd_nonzero, ui_less != 0) {
            let wrapper = Box::new(ImeContext::new(ctx));
            let ptr = Box::into_raw(wrapper);
            log::info!("InputContext created: {:?}", ptr);
            ptr as jlong
        } else {
            log::error!("Failed to create Imm32InputContext");
            0
        }
    }

    #[cfg(not(windows))]
    {
        log::error!("Cannot create InputContext on non-Windows");
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_createInputContextWin32__JI(
    env: JNIEnv,
    _class: JClass,
    hwnd: jlong,
    api: jint,
) -> jlong {
    Java_com_dhj_ingameime_rust_RustImeLibrary_createInputContextWin32__JIZ(
        env, _class, hwnd, api, JNI_FALSE
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_destroyInputContext(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    if ptr != 0 {
        log::info!("Destroying InputContext: {:?}", ptr);
        unsafe {
            let _ = Box::from_raw(ptr as *mut ImeContext);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setInputContextActivated(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    _obj: JObject,
    activated: jboolean,
) {
    if ptr != 0 {
        unsafe {
            let wrapper = &mut *(ptr as *mut ImeContext);
            wrapper.ctx.set_activated(activated != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_isInputContextActivated(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    _obj: JObject,
) -> jboolean {
    if ptr != 0 {
        unsafe {
            let wrapper = &*(ptr as *mut ImeContext);
            if wrapper.ctx.get_activated() { JNI_TRUE } else { JNI_FALSE }
        }
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_getInputMode(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    _obj: JObject,
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
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setPreEditRect(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    _obj: JObject,
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
// Callback registration
// ============================================================================

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setCommitCallback(
    _env: JNIEnv,
    _class: JClass,
    _ptr: jlong,
    _obj: JObject,
    _callback: JObject,
) {
    // Stub - callbacks not yet implemented
    log::warn!("CommitCallback not yet implemented");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setPreEditCallback(
    _env: JNIEnv,
    _class: JClass,
    _ptr: jlong,
    _obj: JObject,
    _callback: JObject,
) {
    // Stub - callbacks not yet implemented
    log::warn!("PreEditCallback not yet implemented");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setCandidateListCallback(
    _env: JNIEnv,
    _class: JClass,
    _ptr: jlong,
    _obj: JObject,
    _callback: JObject,
) {
    // Stub - callbacks not yet implemented
    log::warn!("CandidateListCallback not yet implemented");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dhj_ingameime_rust_RustImeLibrary_setInputModeCallback(
    _env: JNIEnv,
    _class: JClass,
    _ptr: jlong,
    _obj: JObject,
    _callback: JObject,
) {
    // Stub - callbacks not yet implemented
    log::warn!("InputModeCallback not yet implemented");
}
