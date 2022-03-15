use std::path::Path;

use color_eyre::Result;
use rusb::Context;
use zkraken_lib::{open_device, NZXTDevice, PID, VID};

fn main() -> Result<()> {
    let api = hidapi_rusb::HidApi::new()?;
    let mut context = Context::new()?;
    let hid_device = api.open(VID, PID)?;

    // We need to use RUSB as well because HIDAPI doesn't support the writing to BULK endpoint.
    let (_, mut handle) =
        open_device(&mut context, VID, PID).expect("No NZXT Kraken Z device found.");

    let mut nzxt_device = NZXTDevice {
        device: &hid_device,
        bulk_endpoint_handle: &mut handle,
        initialised: false,
        rotation_degrees: 270,
    };

    nzxt_device.initialise()?;

    let firmware = nzxt_device.get_firmware_version()?;
    println!("Firmware version: {}", firmware);

    let status = nzxt_device.get_status()?;
    println!("Status: {:?}", status);

    nzxt_device.set_fan_duty(80)?;
    nzxt_device.set_pump_duty(80)?;

    let image = Path::new("C:\\Users\\me\\Downloads\\elmo.gif");

    nzxt_device.set_image(image, 3, true)?;

    Ok(())
}
