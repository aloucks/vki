use ash::vk;

use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitError {
    Library(String),
    VkResult(vk::Result),
}

impl Display for InitError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl StdError for InitError {}

impl<'a> From<&'a InitError> for InitError {
    fn from(e: &'a InitError) -> InitError {
        e.clone()
    }
}

impl From<ash::InstanceError> for InitError {
    fn from(e: ash::InstanceError) -> InitError {
        match e {
            ash::InstanceError::VkError(e) => InitError::VkResult(e),
            ash::InstanceError::LoadError(v) => InitError::Library(v.join("; ")),
        }
    }
}

impl From<ash::LoadingError> for InitError {
    fn from(e: ash::LoadingError) -> InitError {
        match e {
            ash::LoadingError::LibraryLoadError(e) => InitError::Library(e),
        }
    }
}

impl From<vk::Result> for InitError {
    fn from(e: vk::Result) -> InitError {
        InitError::VkResult(e)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SurfaceError {
    UnsupportedImageUsageFlags(vk::ImageUsageFlags),
    UnsupportedImageTransformFlags(vk::SurfaceTransformFlagsKHR),
    UnsupportedFormat(vk::SurfaceFormatKHR),
    VkError(vk::Result),
}

impl From<vk::Result> for SurfaceError {
    fn from(e: vk::Result) -> SurfaceError {
        SurfaceError::VkError(e)
    }
}

impl Display for SurfaceError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl StdError for SurfaceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncoderError {
    Validation(String),
    VkError(vk::Result),
}

impl From<vk::Result> for EncoderError {
    fn from(e: vk::Result) -> EncoderError {
        EncoderError::VkError(e)
    }
}

impl Display for EncoderError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl StdError for EncoderError {}

#[derive(Clone)]
pub struct Error {
    kind: ErrorKind,
    code: Option<vk::Result>,
}

#[derive(Clone, Debug)]
pub enum ErrorKind {
    VkResult(vk::Result),
}
