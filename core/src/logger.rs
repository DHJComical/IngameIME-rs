//! Logger bridge that forwards Rust logs to Java's Log4j via JNI.

use jni::objects::{Global, JObject, JValue};
use jni::{jni_sig, JavaVM};
use jni::jni_str;
use std::sync::atomic::{AtomicBool, Ordering};

static mut JAVA_VM: Option<JavaVM> = None;
static mut JAVA_LOGGER: Option<Global<JObject<'static>>> = None;
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Initialize the logger with a Java VM and logger reference
pub fn init(java_vm: JavaVM, java_logger: Global<JObject<'static>>) {
    unsafe {
        JAVA_VM = Some(java_vm);
        JAVA_LOGGER = Some(java_logger);
    }
}

/// Set whether debug logging is enabled
pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Check if debug logging is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::SeqCst)
}

/// Log a message at info level
pub fn log_info(message: &str) {
    log_to_java_logger("info", message);
}

/// Log a message at debug level (only if debug logging is enabled)
pub fn log_debug(message: &str) {
    if is_debug_enabled() {
        log_to_java_logger("debug", message);
    }
}

/// Log a message at error level
pub fn log_error(message: &str) {
    log_to_java_logger("error", message);
}

/// Log a message at warn level
pub fn log_warn(message: &str) {
    log_to_java_logger("warn", message);
}

/// Internal function to forward log to Java's Log4j
fn log_to_java_logger(level: &str, message: &str) {
    unsafe {
        let Some(ref vm) = JAVA_VM else {
            // Fallback: print to stderr if Java VM not available
            eprintln!("[IngameIME-Rust] {}", message);
            return;
        };
        let Some(ref logger) = JAVA_LOGGER else {
            // Fallback: print to stderr if Java Logger not available
            eprintln!("[IngameIME-Rust] {}", message);
            return;
        };

        let formatted = format!("[IngameIME-Rust] {}", message);
        
        let _: Result<(), jni::errors::Error> = vm.attach_current_thread(|env| {
            if let Ok(jmsg) = env.new_string(&formatted) {
                let method = match level {
                    "error" => jni_str!("error"),
                    "warn" => jni_str!("warn"),
                    "info" => jni_str!("info"),
                    "debug" => jni_str!("debug"),
                    _ => jni_str!("info"),
                };

                let _ = env.call_method(
                    logger,
                    method,
                    jni_sig!((java.lang.String) -> void),
                    &[JValue::Object(&jmsg)],
                );
            }
            Ok(())
        });
    }
}
