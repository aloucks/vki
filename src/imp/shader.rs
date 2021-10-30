
use ash::vk;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{DeviceInner, ShaderModuleInner};
use crate::{Error, ShaderModule, ShaderModuleDescriptor};

use std::sync::Arc;
use std::{mem, ptr};

impl ShaderModuleInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: ShaderModuleDescriptor) -> Result<ShaderModuleInner, Error> {
        // TODO: Use spirv-cross to reflect binding and attribute info so we can determine
        //       when a shader module is compatible with a given pipeline layout

        // Copy the code to a temp buffer to guarantee alignment and zero padding
        let byte_count = descriptor.code.len();
        let word_extra = if byte_count % 4 > 0 { 1 } else { 0 };
        let word_count = ((byte_count / mem::size_of::<u32>()) + word_extra).max(1);
        let mut words: Vec<u32> = vec![0; word_count as usize];

        unsafe {
            ptr::copy_nonoverlapping(descriptor.code.as_ptr(), words.as_mut_ptr() as *mut u8, byte_count);
        }

        let create_info = vk::ShaderModuleCreateInfo {
            code_size: byte_count,
            p_code: words.as_ptr(),
            ..Default::default()
        };

        let handle = unsafe { device.raw.create_shader_module(&create_info, None)? };

        Ok(ShaderModuleInner { handle, device })
    }
}

impl Into<ShaderModule> for ShaderModuleInner {
    fn into(self) -> ShaderModule {
        ShaderModule { inner: Arc::new(self) }
    }
}

impl Drop for ShaderModuleInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}
