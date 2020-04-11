use vki::Instance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instance = Instance::new()?;

    for adapter in instance.enumerate_adapters()?.iter() {
        println!("{} ({:?})", adapter.name(), adapter.properties().device_type);
    }

    Ok(())
}
