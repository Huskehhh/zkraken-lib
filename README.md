# zkraken-lib

A cross-platform driver library for the Kraken Z series AIO coolers written in Rust.

This project was made a lot easier referencing the work done by [liquidctl](https://github.com/liquidctl/liquidctl/), [KrakenZPlayground](https://github.com/ProtozeFOSS/KrakenZPlayground) and [rcue](https://github.com/mygnu/rcue/)

## Example usage

```rust
use std::path::Path;

use color_eyre::Result;
use rusb::{open_device_with_vid_pid, set_log_level};
use zkraken_lib::{NZXTDevice, PID, VID};

fn main() -> Result<()> {
    #[cfg(debug_assertions)]
    set_log_level(rusb::LogLevel::Debug);
    
    // We need to use RUSB as well because HIDAPI doesn't support the writing to BULK endpoint.
    let mut handle = open_device_with_vid_pid(VID, PID).expect("No Kraken Z device found!");
    let api = hidapi_rusb::HidApi::new()?;
    let hid_device = api.open(VID, PID)?;
    let nzxt_device = NZXTDevice::new(&hid_device, &mut handle, 270)?;

    let firmware = nzxt_device.get_firmware_version()?;
    println!("Firmware version: {}", firmware);

    let status = nzxt_device.get_status()?;
    println!("Status: {:?}", status);

    nzxt_device.set_fan_duty(80)?;
    nzxt_device.set_pump_duty(80)?;

    let image = Path::new("duck.jpg");
    nzxt_device.set_image(image, 1, true)?;

    Ok(())
}
```

# Disclaimer

I provide no guarantees or warranties for the code or the functionality provided. **Use at your own risk.**