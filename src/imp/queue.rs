use ash::vk;

use crate::{Queue, SwapchainImage};

impl<'a> Queue<'a> {
    pub fn present(self, frame: SwapchainImage) -> Result<(), vk::Result> {
        {
            let device = &frame.swapchain.device;
            let mut state = frame.swapchain.device.state.lock();
            let command_buffer = state.get_pending_command_buffer(&device)?;
            let texture = &frame.swapchain.textures[frame.image_index as usize];
            texture.transition_usage_now(command_buffer, texture.descriptor.usage)?;
            state.submit_pending_commands(&frame.swapchain.device, *self.inner)?;

            // these should always be empty after pending commands were submitted
            debug_assert_eq!(0, state.get_wait_semaphores().len());
        }

        let image_indices = [frame.image_index];
        let swapchains = [frame.swapchain.handle];
        let present_info = vk::PresentInfoKHR::builder()
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            let suboptimal = frame
                .swapchain
                .device
                .raw_ext
                .swapchain
                .queue_present(self.inner.handle, &present_info)?;
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
