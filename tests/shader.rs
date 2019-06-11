use std::borrow::Cow;
use vki::ShaderModuleDescriptor;

pub mod support;

#[test]
fn create_shader_module() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;
        let descriptor = ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/shader.vert.spv")),
        };
        let _shader_module = device.create_shader_module(&descriptor)?;
        Ok(instance)
    });
}
