use vki::Instance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = pretty_env_logger::try_init();

    let instance = Instance::new()?;

    println!("Instance version: {:?}", instance.version());
    println!();

    for adapter in instance.enumerate_adapters()?.iter() {
        let properties = adapter.properties();
        println!("Adapter Name:   {:?}", adapter.name());
        println!("API Version:    {:?}", properties.api_version);
        println!("Driver Version: {:?} ({})", properties.driver_version_string(), properties.driver_version);
        println!("Device Type:    {:?}", properties.device_type);
        println!("Device ID:      {:?}", properties.device_id);
        println!("Vendor ID:      {:?}", properties.vender_id);
        println!();
    }

    Ok(())
}
