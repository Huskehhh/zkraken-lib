# zkraken-lib

A cross-platform driver library for the Kraken Z series AIO coolers written in Rust.

This project was made a lot easier referencing the work done by [liquidctl](https://github.com/liquidctl/liquidctl/), [KrakenZPlayground](https://github.com/ProtozeFOSS/KrakenZPlayground) and [rcue](https://github.com/mygnu/rcue/)

## Example usage

```rust
use std::path::Path;

use color_eyre::Result;
use rusb::open_device_with_vid_pid;
use zkraken_lib::{NZXTDevice, PID, VID};

fn main() -> Result<()> {
    let mut handle = open_device_with_vid_pid(VID, PID).expect("No Kraken Z device found!");
    let nzxt_device = NZXTDevice::new(&mut handle, 270)?;

    let status = nzxt_device.get_status()?;
    let firmware_version = nzxt_device.get_firmware_version()?;

    println!("Status: {:?}", status);
    println!("Firmware version: {}", firmware_version);

    nzxt_device.set_fan_duty(80)?;
    nzxt_device.set_pump_duty(80)?;

    let image = Path::new("elmo.png");
    nzxt_device.set_image(image, 1, true)?;

    Ok(())
}
```

# Disclaimer

I provide no guarantees or warranties for the code or the functionality provided. **Use at your own risk.**