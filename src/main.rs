use std::path::Path;

use color_eyre::Result;
use zkraken_lib::{NZXTDevice, PID, VID};

fn main() -> Result<()> {
    let api = hidapi_rusb::HidApi::new()?;
    let device = api.open(VID, PID)?;

    let mut nzxt_device = NZXTDevice {
        device: &device,
        initialised: false,
        rotation_degrees: 270,
    };

    nzxt_device.initialise()?;

    let firmware = nzxt_device.get_firmware_version()?;
    println!("Firmware version: {}", firmware);

    let status = nzxt_device.get_status()?;
    println!("Status: {:?}", status);

    nzxt_device.set_fan_duty(80)?;

    let image = Path::new("C:\\Users\\me\\Documents\\zkraken-lib\\elmo.gif");

    nzxt_device.set_image(image, 3, true)?;

    Ok(())
}
