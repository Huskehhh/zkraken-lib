# zkraken-lib

A cross-platform driver library for the Kraken Z series AIO coolers written in Rust.

This project was made a lot easier referencing the work done by [liquidctl](https://github.com/liquidctl/liquidctl/), [KrakenZPlayground](https://github.com/ProtozeFOSS/KrakenZPlayground) and [rcue](https://github.com/mygnu/rcue/)

## Example usage

```rust
use color_eyre::Result;
use zkraken_lib::{NZXTDevice, PID, VID};

fn main() -> Result<()> {
    let api = hidapi::HidApi::new()?;
    let device = api.open(VID, PID)?;

    let mut nzxt_device = NZXTDevice {
        device: &device,
        initialised: false,
    };

    nzxt_device.initialise()?;

    let firmware = nzxt_device.get_firmware_version()?;
    println!("Firmware version: {}", firmware);

    nzxt_device.set_brightness(100)?;
    nzxt_device.set_fan_duty(80)?;
    nzxt_device.set_pump_duty(80)?;

    let status = nzxt_device.get_status()?;

    println!("Status: {:?}", status);

    Ok(())
}
```

# Disclaimer

I provide no guarantees or warranties for the code or the functionality provided. **Use at your own risk.**