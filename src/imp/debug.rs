use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::mem;
use std::sync::atomic::AtomicBool;

use ash::version::InstanceV1_0;
use ash::vk;

use parking_lot::Mutex;
use std::fmt::{Debug, Display};

use crate::Instance;
use std::sync::atomic::Ordering;

#[allow(dead_code)]
#[allow(unused_variables)]
pub unsafe extern "system" fn debug_utils_messenger_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    callback: *const vk::DebugUtilsMessengerCallbackDataEXT,
    userdata: *mut c_void,
) -> u32 {
    if callback.is_null() {
        log::warn!("debug message callback was null");
    } else {
        let callback = *callback;

        let message_id = callback.message_id_number;
        let message_id_name = CStr::from_ptr(callback.p_message_id_name).to_string_lossy();

        let message = CStr::from_ptr(callback.p_message).to_string_lossy();

        let level = match severity {
            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::Level::Trace,
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::Level::Warn,
            vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::Level::Info,
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::Level::Error,
            _ => log::Level::Debug,
        };

        log::log!(level, "[{:?}] {}", ty, message);
    }

    unimplemented!()
    //vk::FALSE
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub unsafe extern "system" fn debug_report_callback(
    flags: vk::DebugReportFlagsEXT,
    _object_type: vk::DebugReportObjectTypeEXT,
    _object: u64,
    _location: usize,
    message_code: i32,
    _layer_prefix: *const libc::c_char,
    message: *const libc::c_char,
    _userdata: *mut libc::c_void,
) -> u32 {
    let message = CStr::from_ptr(message).to_string_lossy();

    let level = match flags {
        vk::DebugReportFlagsEXT::DEBUG => log::Level::Trace,
        vk::DebugReportFlagsEXT::WARNING => log::Level::Warn,
        vk::DebugReportFlagsEXT::PERFORMANCE_WARNING => log::Level::Info,
        vk::DebugReportFlagsEXT::INFORMATION => log::Level::Debug,
        vk::DebugReportFlagsEXT::ERROR => log::Level::Error,
        _ => log::Level::Debug,
    };

    log::log!(level, "[{}] {}", message_code, message);

    vk::FALSE
}

pub struct ValidationError {
    flags: vk::DebugReportFlagsEXT,
    message: String,
}

impl Debug for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.flags, self.message)
    }
}

pub struct ValidationErrors(pub Vec<ValidationError>);

impl Debug for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for err in self.0.iter() {
            writeln!(f, "\n {:?}", err)?;
        }
        Ok(())
    }
}

impl Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for err in self.0.iter() {
            writeln!(f, "\n {:?}", err)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

#[allow(dead_code)]
#[allow(unused_variables)]
pub unsafe extern "system" fn debug_report_callback_test(
    flags: vk::DebugReportFlagsEXT,
    _object_type: vk::DebugReportObjectTypeEXT,
    _object: u64,
    _location: usize,
    message_code: i32,
    _layer_prefix: *const libc::c_char,
    message: *const libc::c_char,
    userdata: *mut libc::c_void,
) -> u32 {
    let message = CStr::from_ptr(message).to_string_lossy().to_string();
    let handle: vk::Instance = mem::transmute(userdata);
    let mut errors = ERRORS.lock();
    let errors = errors.entry(handle).or_default();
    errors.push(ValidationError { message, flags });
    vk::FALSE
}

lazy_static::lazy_static! {
    static ref ERRORS: Mutex<HashMap<vk::Instance, Vec<ValidationError>>> = {
        Mutex::new(HashMap::new())
    };
}

lazy_static::lazy_static! {
    pub static ref TEST_VALIDATION_HOOK: AtomicBool = AtomicBool::new(false);
}

pub fn validate<F>(f: F)
where
    F: FnOnce() -> Result<Instance, Box<std::error::Error>>,
{
    TEST_VALIDATION_HOOK.store(true, Ordering::Release);
    let instance = f().unwrap();
    let mut errors = ERRORS.lock();
    let errors = errors.remove(&instance.inner.raw.handle());
    if let Some(errors) = errors {
        println!("error count: {}", errors.len());
        panic!("{:?}", ValidationErrors(errors));
    }
}
