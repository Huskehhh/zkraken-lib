use std::path::Path;
use std::time::Duration;

use color_eyre::eyre::eyre;
use color_eyre::eyre::Result;
use hidapi_rusb::HidDevice;
use image::GenericImageView;
use rusb::DeviceHandle;
use rusb::UsbContext;

pub const PUMP_ENDPOINT_ADDRESS: u8 = 0x1;
pub const FAN_ENDPOINT_ADDRESS: u8 = 0x2;

pub const WRITE_LENGTH: usize = 64;
pub const BULK_WRITE_LENGTH: usize = 512;

pub const READ_LENGTH: usize = 64;
pub const BULK_READ_LENGTH: usize = 512;

pub const BULK_TIMEOUT: Duration = std::time::Duration::from_secs(10);

// Kraken Z series.
pub const VID: u16 = 0x1e71;
pub const PID: u16 = 0x3008;

const SETUP_BUCKET: u8 = 0x32;
const SET_BUCKET: u8 = 0x1;
const DELETE_BUCKET: u8 = 0x2;
const QUERY_BUCKET: u8 = 0x30;
const SWITCH_BUCKET: u8 = 0x38;

const WRITE_SETUP: u8 = 0x36;
const WRITE_START: u8 = 0x1;
const WRITE_FINISH: u8 = 0x2;

const BULK_WRITE_ENDPOINT: u8 = 0x02;

pub struct NZXTDevice<'a, T: UsbContext> {
    pub device: &'a HidDevice,
    pub bulk_endpoint_handle: &'a mut DeviceHandle<T>,
    pub rotation_degrees: i32,
}

#[derive(Debug)]
pub struct DeviceStatus {
    pub temp: i32,
    pub pump_rpm: i32,
    pub pump_duty: i32,
    pub fan_rpm: i32,
    pub fan_duty: i32,
}

impl<T: UsbContext> NZXTDevice<'_, T> {
    /// Create an instance of NZXTDevice.
    pub fn new<'a>(
        device: &'a HidDevice,
        bulk_endpoint_handle: &'a mut DeviceHandle<T>,
        rotation_degrees: i32,
    ) -> Result<NZXTDevice<'a, T>> {
        // Set auto detach kernel driver.
        bulk_endpoint_handle.set_auto_detach_kernel_driver(true)?;

        // Claim the interface that the BULK endpoint is on.
        bulk_endpoint_handle.claim_interface(0)?;

        let mut nzxt_device = NZXTDevice {
            device,
            bulk_endpoint_handle,
            rotation_degrees,
        };

        // Init the device.
        nzxt_device.initialise()?;

        Ok(nzxt_device)
    }

    /// Initialise the NZXT device.
    pub fn initialise(&mut self) -> Result<()> {
        self.write(&[0x70, 0x01])?;

        self.clear_read_buffer()?;

        Ok(())
    }

    /// Write INTERRUPT raw bytes to the NZXT device.
    fn write(&self, data: &[u8]) -> Result<()> {
        let mut buf = [0u8; WRITE_LENGTH];
        buf.fill(0x0);

        // Copy the data to a new buffer with the correct length.
        if data.len() > WRITE_LENGTH {
            buf.copy_from_slice(data);
        }

        // Write to the USB device via the endpoint.
        self.device.write(data)?;

        Ok(())
    }

    /// Write BULK raw bytes to the NZXT device.
    fn write_bulk(&self, data: &[u8]) -> Result<()> {
        let mut buf = [0u8; BULK_WRITE_LENGTH];
        buf.fill(0x0);

        buf.copy_from_slice(data);

        // Write to the USB device via the endpoint.
        self.bulk_endpoint_handle
            .write_bulk(0x02, &buf, BULK_TIMEOUT)?;

        Ok(())
    }

    /// Read 64 bytes (READ_LENGTH) from the device.
    fn read(&self) -> Result<Vec<u8>> {
        let mut buf = [0u8; READ_LENGTH];

        Ok(self.device.read(&mut buf).map(|_| buf.to_vec())?)
    }

    /// Force clear the previous read buffer so that new data can be reported.
    fn clear_read_buffer(&self) -> Result<()> {
        let mut buf = [0u8; 1];

        while self.device.read_timeout(&mut buf, 3)? > 0 {
            // Do nothing.
        }

        Ok(())
    }

    /// Return the status of the device.
    pub fn get_status(&self) -> Result<DeviceStatus> {
        self.clear_read_buffer()?;

        self.write(&[0x74, 0x01])?;
        let data = self.read()?;

        // Liquid temp.
        let temp = (data[15] + data[16] / 10) as i32;
        let pump_rpm = ((data[18] as i32) << 8) | (data[17] as i32);
        let pump_duty = data[19] as i32;
        let fan_rpm = ((data[24] as i32) << 8) | (data[23] as i32);
        let fan_duty = data[25] as i32;

        let status = DeviceStatus {
            temp,
            pump_rpm,
            pump_duty,
            fan_rpm,
            fan_duty,
        };

        Ok(status)
    }

    /// Set the pump duty.
    pub fn set_pump_duty(&self, duty: u8) -> Result<()> {
        self.set_duty(duty, PUMP_ENDPOINT_ADDRESS)
    }

    /// Set the fan duty.
    pub fn set_fan_duty(&self, duty: u8) -> Result<()> {
        self.set_duty(duty, FAN_ENDPOINT_ADDRESS)
    }

    /// Set the given duty for the specified endpoint.
    fn set_duty(&self, duty: u8, address: u8) -> Result<()> {
        if (20..=100).contains(&duty) {
            let mut buffer = [0u8; WRITE_LENGTH];

            buffer.fill(0x0);

            // SET DUTY
            buffer[0] = 0x72;

            // Either PUMP or FAN
            buffer[1] = address;

            // From index 4 to 43 needs to be set to the duty.
            (4..44).for_each(|i| {
                buffer[i] = duty;
            });

            self.write(&buffer)?;

            return Ok(());
        }

        Err(eyre!("Duty value is out of bounds"))
    }

    /// Get the firmware version from the device.
    pub fn get_firmware_version(&self) -> Result<String> {
        self.clear_read_buffer()?;

        // Request firmware info.
        self.write(&[0x10, 0x01])?;

        // Read the response.
        let data = self.read()?;

        // Parse into version string.
        Ok(parse_firmware_info(&data))
    }

    /// Set the visual mode for the device.
    pub fn set_visual_mode(&self, mode: u8, index: u8) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = SWITCH_BUCKET;
        buffer[1] = 0x1;
        buffer[2] = mode;
        buffer[3] = index;

        self.write(&buffer)?;

        Ok(())
    }

    /// Switch to custom bucket.
    pub fn switch_bucket(&self, index: u8) -> Result<()> {
        self.set_visual_mode(4, index)
    }

    pub fn delete_all_buckets(&self) -> Result<()> {
        for i in 0..15 {
            self.delete_bucket(i)?;
        }

        Ok(())
    }

    /// Send query bucket for given index.
    pub fn send_query_bucket(&self, index: u8) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = QUERY_BUCKET;
        buffer[1] = 0x04;
        buffer[3] = index;

        self.write(&buffer)?;

        Ok(())
    }

    /// Clear the memory bucket at given index.
    pub fn delete_bucket(&self, index: u8) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = SETUP_BUCKET;
        buffer[1] = DELETE_BUCKET;
        buffer[2] = index;

        self.write(&buffer)?;

        Ok(())
    }

    /// Set the device LCD to display liquid temperature.
    pub fn set_liquid_temp_mode(&self) -> Result<()> {
        self.set_visual_mode(2, 0)
    }

    /// Set the device LCD to display a blank screen.
    pub fn set_blank_screen(&self) -> Result<()> {
        self.set_visual_mode(0, 0)
    }

    /// Set the device LCD to display the dual infographic (CPU & GPU temp).
    pub fn set_dual_infographic_mode(&self) -> Result<()> {
        self.set_visual_mode(4, 0)
    }

    /// Set the device LCD brightness.
    pub fn set_brightness(&self, brightness: u8) -> Result<()> {
        if (0..=100).contains(&brightness) {
            let mut buffer = [0u8; WRITE_LENGTH];
            buffer.fill(0x0);

            // QUERY
            buffer[0] = 0x30;

            // BRIGHTNESS
            buffer[1] = 0x2;

            // SET
            buffer[2] = 0x1;

            buffer[3] = brightness;

            self.write(&buffer)?;

            return Ok(());
        }

        Err(eyre!("Duty value is out of bounds"))
    }

    pub fn set_image(
        &self,
        path_to_image: &Path,
        index: u8,
        apply_image_after_upload: bool,
    ) -> Result<()> {
        let mut img = image::open(path_to_image)?;

        let (width, height) = img.dimensions();

        if self.rotation_degrees == 90 {
            img = img.rotate90();
        } else if self.rotation_degrees == 180 {
            img = img.rotate180();
        } else if self.rotation_degrees == 270 {
            img = img.rotate270();
        }

        if width != 320 && height != 320 {
            img = img.resize_exact(320, 320, image::imageops::FilterType::Gaussian);
        }

        let image_bytes = img.to_rgba8().into_raw();
        let image_size_bytes = image_bytes.len() as i32;

        self.upload_image(
            &image_bytes,
            image_size_bytes,
            index,
            apply_image_after_upload,
        )?;

        Ok(())
    }

    fn upload_image(
        &self,
        image_bytes: &[u8],
        image_size_bytes: i32,
        index: u8,
        apply_image_after_upload: bool,
    ) -> Result<()> {
        self.set_blank_screen()?;
        self.delete_bucket(index)?;

        let memory_slot = 800 * index as u16;
        // Memory slots are in 1kb sections
        let memory_slot_count = (image_size_bytes / 1024) as u16;

        self.setup_bucket(index, index + 1, memory_slot, memory_slot_count)?;
        self.write_start_bucket(index)?;
        self.send_bulk_data_info(2)?;

        // Write image bytes to BULK endpoint.
        self.bulk_endpoint_handle
            .write_bulk(BULK_WRITE_ENDPOINT, image_bytes, BULK_TIMEOUT)?;

        self.write_finish_bucket(index)?;

        if apply_image_after_upload {
            self.switch_bucket(index)?;
        }

        Ok(())
    }

    /// Set up bucket for uploading an image.
    pub fn setup_bucket(
        &self,
        index: u8,
        id: u8,
        memory_slot: u16,
        memory_slot_count: u16,
    ) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = SETUP_BUCKET;
        buffer[1] = SET_BUCKET;
        buffer[2] = index;
        buffer[3] = id;
        buffer[4] = (memory_slot >> 8) as u8;
        buffer[5] = memory_slot as u8;
        buffer[6] = memory_slot_count as u8;
        buffer[7] = (memory_slot_count >> 8) as u8;
        buffer[8] = 1;

        self.write(&buffer)?;

        Ok(())
    }

    /// Send the write start to given bucket.
    pub fn write_start_bucket(&self, index: u8) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = WRITE_SETUP;
        buffer[1] = WRITE_START;
        buffer[2] = index;

        self.write(&buffer)?;

        Ok(())
    }

    pub fn write_finish_bucket(&self, index: u8) -> Result<()> {
        let mut buffer = [0u8; WRITE_LENGTH];

        buffer.fill(0x0);

        buffer[0] = WRITE_SETUP;
        buffer[1] = WRITE_FINISH;
        buffer[2] = index;

        self.write(&buffer)?;

        Ok(())
    }

    pub fn send_bulk_data_info(&self, mode: u8) -> Result<()> {
        let mut buffer = [0u8; BULK_WRITE_LENGTH];
        buffer.fill(0x0);

        // Fill with 12fa01e8abcdef987654321 (magic numbers) and then mode.
        buffer[0] = 0x12;
        buffer[1] = 0xfa;
        buffer[2] = 0x01;
        buffer[3] = 0xe8;
        buffer[4] = 0xab;
        buffer[5] = 0xcd;
        buffer[6] = 0xef;
        buffer[7] = 0x98;
        buffer[8] = 0x76;
        buffer[9] = 0x54;
        buffer[10] = 0x32;
        buffer[11] = 0x10;
        buffer[12] = mode;
        // More magic?
        buffer[17] = 0x40;
        buffer[18] = 0x96;

        self.write_bulk(&buffer)?;

        Ok(())
    }
}

impl<T: UsbContext> Drop for NZXTDevice<'_, T> {
    fn drop(&mut self) {
        self.bulk_endpoint_handle
            .release_interface(0)
            .expect("Error releasing interface 0 for NZXTDevice");
    }
}

/// Parse the returned data bytes from the device into a firmware version.
fn parse_firmware_info(data: &[u8]) -> String {
    let major = data[0x11];
    let minor = data[0x12];
    let patch = data[0x13];

    format!("version {}.{}.{}", major, minor, patch)
}
