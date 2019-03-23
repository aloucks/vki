use vki::{DeviceDescriptor, Instance, PowerPreference, RequestAdapterOptions};

#[test]
fn instance_new() {
    vki::validate(|| {
        let instance = Instance::new()?;

        Ok(instance)
    });
}

#[test]
fn instance_request_adapter() {
    vki::validate(|| {
        let instance = Instance::new()?;

        let options = RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
        };
        let adapter = instance.request_adaptor(options)?;
        assert!(!adapter.name().is_empty());

        let options = RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
        };
        let adapter = instance.request_adaptor(options)?;
        assert!(!adapter.name().is_empty());

        Ok(instance)
    });
}

#[test]
fn instance_create_device() {
    vki::validate(|| {
        let instance = Instance::new()?;
        let options = RequestAdapterOptions::default();
        let adapter = instance.request_adaptor(options)?;
        let _device = adapter.create_device(DeviceDescriptor::default())?;

        Ok(instance)
    });
}
