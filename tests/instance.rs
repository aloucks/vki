use vki::{AdapterOptions, DeviceDescriptor, Instance, PowerPreference};

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

        let options = AdapterOptions {
            power_preference: PowerPreference::HighPerformance,
        };
        let adapter = instance.request_adapter(options)?;
        assert!(!adapter.name().is_empty());

        let options = AdapterOptions {
            power_preference: PowerPreference::LowPower,
        };
        let adapter = instance.request_adapter(options)?;
        assert!(!adapter.name().is_empty());

        Ok(instance)
    });
}

#[test]
fn instance_enumerate_adapters() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;
        let adapters = instance.enumerate_adapters()?;
        assert!(!adapters.is_empty(), "no adapters were found");
        assert!(!adapters[0].name().is_empty());
        Ok(instance)
    });
}

#[test]
fn instance_create_device() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;
        let options = AdapterOptions::default();
        let adapter = instance.request_adapter(options)?;
        let _device = adapter.create_device(DeviceDescriptor::default())?;

        Ok(instance)
    });
}
