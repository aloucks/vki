use vki::SamplerDescriptor;

pub mod support;

#[test]
fn create_sampler() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;
        let descriptor = SamplerDescriptor::default();
        let _sampler = device.create_sampler(descriptor)?;
        Ok(instance)
    });
}
