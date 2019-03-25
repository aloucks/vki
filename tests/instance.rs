use vki::{DeviceDescriptor, Instance, PowerPreference, RequestAdapterOptions};

#[test]
fn instance_new() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;

        Ok(instance)
    });
}

#[test]
fn instance_request_adapter() {
    let _ = pretty_env_logger::try_init();
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
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;
        let options = RequestAdapterOptions::default();
        let adapter = instance.request_adaptor(options)?;
        let _device = adapter.create_device(DeviceDescriptor::default())?;

        Ok(instance)
    });
}
