use ash::vk;

use crate::imp::TextureInner;

impl TextureInner {
    fn transition_usage() -> Result<(), vk::Result> {
        Ok(())
    }
}
