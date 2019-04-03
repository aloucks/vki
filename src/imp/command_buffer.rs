use ash::vk;

use crate::imp::command::Command;
use crate::imp::pass_resource_usage::CommandBufferResourceUsage;
use crate::imp::CommandBufferInner;

pub struct CommandBufferState {
    pub commands: Vec<Command>,
    pub resource_usages: CommandBufferResourceUsage,
}

impl CommandBufferInner {
    pub fn record_commands(&self, _command_buffer: vk::CommandBuffer) {
        // let mut next_pass_number = 0;
        for command in self.state.commands.iter() {
            match command {
                _ => panic!(),
            }
        }
    }
}
