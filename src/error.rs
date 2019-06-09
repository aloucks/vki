use ash::vk;

use backtrace::Backtrace;

use std::error::Error as StdError;
use std::fmt::{self, Display};

pub use vk::Result as VkResult;

impl<'a> From<&'a Error> for Error {
    fn from(e: &'a Error) -> Error {
        e.clone()
    }
}

impl From<ash::InstanceError> for Error {
    fn from(e: ash::InstanceError) -> Error {
        match e {
            ash::InstanceError::VkError(e) => Error::from(e),
            ash::InstanceError::LoadError(v) => Error::from(format!("Failed to load vulkan library: {}", v.join("; "))),
        }
    }
}

impl From<ash::LoadingError> for Error {
    fn from(e: ash::LoadingError) -> Error {
        match e {
            ash::LoadingError::LibraryLoadError(e) => Error::from(e),
        }
    }
}

impl From<vk_mem::Error> for Error {
    fn from(e: vk_mem::Error) -> Error {
        match e.kind() {
            vk_mem::ErrorKind::Vulkan(r) => Error::from(*r),
            vk_mem::ErrorKind::Memory(s) => Error::from(s.clone()),
            vk_mem::ErrorKind::Parse(s) => Error::from(s.clone()),
            vk_mem::ErrorKind::Path(p) => Error::from(format!("{:?}", p)),
            vk_mem::ErrorKind::Bug(s) => Error::from(s.clone()),
            vk_mem::ErrorKind::Config(s) => Error::from(s.clone()),
            vk_mem::ErrorKind::Io => Error::from("VMA: I/O error"),
            vk_mem::ErrorKind::Number => Error::from("VMA: number parse error"),
            _ => Error::from("VMA: unknown memory operation error"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Error {
    kind: ErrorKind,
    backtrace: Option<Backtrace>,
}

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        // ignore the backtrace
        self.kind.eq(&other.kind)
    }
}

impl Eq for Error {}

fn backtrace() -> Option<Backtrace> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Once;

    static ENABLED: AtomicBool = AtomicBool::new(false);
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let enabled = std::env::var("RUST_BACKTRACE")
            .map(|v| v != "0" && v != "false")
            .unwrap_or(false);
        ENABLED.store(enabled, Ordering::Relaxed);
    });

    if ENABLED.load(Ordering::Relaxed) {
        Some(Backtrace::new())
    } else {
        None
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Error {
        Error {
            kind: ErrorKind::Message(msg),
            backtrace: backtrace(),
        }
    }
}

impl<'a> From<&'a str> for Error {
    fn from(msg: &'a str) -> Error {
        Error {
            kind: ErrorKind::Message(msg.to_owned()),
            backtrace: backtrace(),
        }
    }
}

impl From<vk::Result> for Error {
    fn from(code: vk::Result) -> Error {
        Error {
            kind: ErrorKind::Code(code),
            backtrace: backtrace(),
        }
    }
}

impl Into<vk::Result> for Error {
    fn into(self) -> vk::Result {
        match self.kind() {
            ErrorKind::Message(_) => vk::Result::ERROR_VALIDATION_FAILED_EXT,
            ErrorKind::Code(code) => *code,
        }
    }
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Set `RUST_BACKTRACE=1` to enable backtraces
    pub fn backtrace(&self) -> Option<&Backtrace> {
        self.backtrace.as_ref()
    }
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Code(VkResult),
    Message(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwapchainError {
    OutOfDate,
    Other(Error),
}

impl From<SwapchainError> for Error {
    fn from(e: SwapchainError) -> Error {
        match e {
            SwapchainError::OutOfDate => Error::from(vk::Result::ERROR_OUT_OF_DATE_KHR),
            SwapchainError::Other(e) => e,
        }
    }
}

impl From<Error> for SwapchainError {
    fn from(e: Error) -> SwapchainError {
        match e.kind {
            ErrorKind::Code(vk::Result::ERROR_OUT_OF_DATE_KHR) => SwapchainError::OutOfDate,
            ErrorKind::Code(code) => SwapchainError::Other(Error::from(code)),
            ErrorKind::Message(msg) => SwapchainError::Other(Error::from(msg)),
        }
    }
}

impl From<vk::Result> for SwapchainError {
    fn from(e: vk::Result) -> SwapchainError {
        match e {
            vk::Result::ERROR_OUT_OF_DATE_KHR => SwapchainError::OutOfDate,
            code => SwapchainError::Other(Error::from(code)),
        }
    }
}

impl StdError for SwapchainError {}

impl Display for SwapchainError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FenceError {
    Timeout,
    Other(Error),
}

impl From<FenceError> for Error {
    fn from(e: FenceError) -> Error {
        match e {
            FenceError::Timeout => Error::from(vk::Result::TIMEOUT),
            FenceError::Other(e) => e,
        }
    }
}

impl From<Error> for FenceError {
    fn from(e: Error) -> FenceError {
        match e.kind {
            ErrorKind::Code(vk::Result::TIMEOUT) => FenceError::Timeout,
            ErrorKind::Code(code) => FenceError::Other(Error::from(code)),
            ErrorKind::Message(msg) => FenceError::Other(Error::from(msg)),
        }
    }
}

impl From<vk::Result> for FenceError {
    fn from(e: vk::Result) -> FenceError {
        match e {
            vk::Result::TIMEOUT => FenceError::Timeout,
            code => FenceError::Other(Error::from(code)),
        }
    }
}

impl StdError for FenceError {}

impl Display for FenceError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
