use ash::vk;

use crate::{Queue, SwapchainImage};

impl<'a> Queue<'a> {
    pub fn present(self, frame: SwapchainImage) -> Result<(), vk::Result> {
        let mut wait_semaphores = vec![];

        {
            let device = &frame.swapchain.device;
            let mut state = frame.swapchain.device.state.lock();
            let command_buffer = state.get_pending_command_buffer(&device)?;
            let texture = &frame.swapchain.textures[frame.image_index as usize];
            texture.transition_usage(command_buffer, texture.descriptor.usage)?;
            state.submit_pending_commands(&frame.swapchain.device, *self.inner)?;

            // these should always be empty unless no commands were submitted between
            // acquire image and present
            wait_semaphores.extend_from_slice(state.get_wait_semaphores());
            state.delete_when_unused_wait_semaphores();
            log::trace!("wait_semaphores: {:?}", wait_semaphores);
        }

        let image_indices = [frame.image_index];
        let swapchains = [frame.swapchain.handle];
        let create_info = vk::PresentInfoKHR::builder()
            .swapchains(&swapchains)
            .wait_semaphores(&wait_semaphores)
            .image_indices(&image_indices);

        unsafe {
            let suboptimal = frame
                .swapchain
                .device
                .raw_ext
                .swapchain
                .queue_present(self.inner.handle, &create_info)?;
            if suboptimal {
                log::warn!("present: suboptimal")
            }
        }

        frame.swapchain.device.tick()
    }

    pub fn submit(&self, _command_buffer: ()) {
        unimplemented!()
    }
}
