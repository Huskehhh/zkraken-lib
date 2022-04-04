#[cfg(not(target_os = "windows"))]
use std::path::Path;
use std::time::Duration;

use color_eyre::eyre::eyre;
use color_eyre::eyre::Result;
#[cfg(not(target_os = "windows"))]
use image::GenericImageView;
use mockall::*;
use rusb::DeviceHandle;
use rusb::UsbContext;

// Kraken Z series.
pub const VID: u16 = 0x1e71;
pub const PID: u16 = 0x3008;

const PUMP_ENDPOINT_ADDRESS: u8 = 0x1;
const FAN_ENDPOINT_ADDRESS: u8 = 0x2;

const WRITE_LENGTH: usize = 64;
const BULK_WRITE_LENGTH: usize = 512;

const READ_LENGTH: usize = 64;

const WRITE_TIMEOUT: Duration = std::time::Duration::from_secs(10);
const READ_TIMEOUT: Duration = std::time::Duration::from_secs(3);

const SETUP_BUCKET: u8 = 0x32;
const SET_BUCKET: u8 = 0x1;
const DELETE_BUCKET: u8 = 0x2;
const QUERY_BUCKET: u8 = 0x30;
const SWITCH_BUCKET: u8 = 0x38;

const WRITE_SETUP: u8 = 0x36;
const WRITE_START: u8 = 0x1;
const WRITE_FINISH: u8 = 0x2;

const INTERRUPT_WRITE_ENDPOINT: u8 = 0x01;
const INTERRUPT_READ_ENDPOINT: u8 = 0x81;
const BULK_WRITE_ENDPOINT: u8 = 0x02;

#[automock]
pub trait NZXTDeviceHandle {
    fn claim_interface(&mut self, iface: u8) -> crate::Result<()>;
    fn write_interrupt(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> crate::Result<usize>;
    fn write_bulk(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> crate::Result<usize>;
    fn read_interrupt(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> crate::Result<usize>;
    fn release_interface(&mut self, iface: u8) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
    #[cfg(not(target_os = "windows"))]
    fn set_auto_detach_kernel_driver(&mut self, auto_detach: bool) -> Result<()>;
}

impl<T: UsbContext> NZXTDeviceHandle for DeviceHandle<T> {
    fn claim_interface(&mut self, iface: u8) -> crate::Result<()> {
        Ok(self.claim_interface(iface)?)
    }

    fn write_interrupt(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> crate::Result<usize> {
        Ok(self.write_interrupt(endpoint, buf, timeout)?)
    }

    fn write_bulk(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> crate::Result<usize> {
        Ok(self.write_bulk(endpoint, buf, timeout)?)
    }

    fn read_interrupt(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> crate::Result<usize> {
        Ok(self.read_interrupt(endpoint, buf, timeout)?)
    }

    fn release_interface(&mut self, iface: u8) -> Result<()> {
        Ok(self.release_interface(iface)?)
    }

    fn reset(&mut self) -> Result<()> {
        Ok(self.reset()?)
    }

    #[cfg(not(target_os = "windows"))]
    fn set_auto_detach_kernel_driver(&mut self, auto_detach: bool) -> Result<()> {
        Ok(self.set_auto_detach_kernel_driver(auto_detach)?)
    }
}

pub struct NZXTDevice<'a> {
    handle: &'a mut dyn NZXTDeviceHandle,
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

impl NZXTDevice<'_> {
    /// Create an instance of NZXTDevice.
    pub fn new(handle: &mut dyn NZXTDeviceHandle, rotation_degrees: i32) -> Result<NZXTDevice> {
        #[cfg(not(target_os = "windows"))]
        {
            handle.set_auto_detach_kernel_driver(true)?;
            handle.claim_interface(0)?;
        }

        handle.claim_interface(1)?;

        let nzxt_device = NZXTDevice {
            handle,
            rotation_degrees,
        };

        // Throw away the response bytes.
        nzxt_device.read()?;

        Ok(nzxt_device)
    }

    /// Write INTERRUPT raw bytes to the NZXT device.
    fn write(&self, data: &[u8]) -> Result<()> {
        let mut buf = [0u8; WRITE_LENGTH];
        buf.fill(0x0);

        // Copy the data to a new buffer with the correct length.
        if data.len() <= WRITE_LENGTH {
            buf[..data.len()].copy_from_slice(data);
        }

        // Write to the USB device via the endpoint.
        self.handle
            .write_interrupt(INTERRUPT_WRITE_ENDPOINT, &buf, WRITE_TIMEOUT)?;

        Ok(())
    }

    /// Write BULK raw bytes to the NZXT device.
    fn write_bulk(&self, data: &[u8]) -> Result<()> {
        let mut buf = [0u8; BULK_WRITE_LENGTH];
        buf.fill(0x0);

        buf[..data.len()].copy_from_slice(data);

        // Write to the USB device via the endpoint.
        self.handle
            .write_bulk(BULK_WRITE_ENDPOINT, &buf, WRITE_TIMEOUT)?;

        Ok(())
    }

    /// Read 64 bytes (READ_LENGTH) from the device.
    fn read(&self) -> Result<Vec<u8>> {
        let mut buf = [0u8; READ_LENGTH];

        self.handle
            .read_interrupt(INTERRUPT_READ_ENDPOINT, &mut buf, READ_TIMEOUT)
            .map(|_| buf.to_vec())
    }

    /// Return the status of the device.
    pub fn get_status(&self) -> Result<DeviceStatus> {
        self.write(&[0x74, 0x01])?;
        let data = self.read()?;
        parse_status(&data)
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
        // Request firmware info.
        self.write(&[0x10, 0x01])?;

        // Read the response.
        let data = self.read()?;

        // Parse into version string.
        Ok(parse_firmware_info(&data))
    }

    /// Set the visual mode for the device.
    pub fn set_visual_mode(&self, mode: u8, index: u8) -> Result<()> {
        self.write(&[SWITCH_BUCKET, 0x1, mode, index])?;

        Ok(())
    }

    /// Switch to custom bucket.
    pub fn switch_bucket(&self, index: u8) -> Result<()> {
        self.set_visual_mode(4, index)
    }

    /// Delete all memory buckets from the device.
    pub fn delete_all_buckets(&self) -> Result<()> {
        for i in 0..15 {
            self.delete_bucket(i)?;
        }

        Ok(())
    }

    /// Send query bucket for given index.
    pub fn send_query_bucket(&self, index: u8) -> Result<()> {
        self.write(&[QUERY_BUCKET, 0x04, 0x00, index])
    }

    /// Clear the memory bucket at given index.
    pub fn delete_bucket(&self, index: u8) -> Result<()> {
        self.write(&[SETUP_BUCKET, DELETE_BUCKET, index])
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
            return self.write(&[0x30, 0x2, 0x1, brightness]);
        }

        Err(eyre!("Duty value is out of bounds"))
    }

    /// Set the device LCD to an image. Will be resized if it does not have height or width of 320px
    /// Will rotate to the NZXTDevice rotation_degrees amount prior to uploading.
    /// Does NOT support gif.
    #[cfg(not(target_os = "windows"))]
    pub fn set_image(
        &self,
        path_to_image: &Path,
        index: u8,
        apply_after_upload: bool,
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

        self.upload_image(&image_bytes, image_size_bytes, index, apply_after_upload)
    }

    /// Upload an image (either still or gif) to the device.
    #[cfg(not(target_os = "windows"))]
    fn upload_image(
        &self,
        image_bytes: &[u8],
        image_size_bytes: i32,
        index: u8,
        apply_after_upload: bool,
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
        self.handle
            .write_bulk(BULK_WRITE_ENDPOINT, image_bytes, WRITE_TIMEOUT)?;

        self.write_finish_bucket(index)?;

        if apply_after_upload {
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
        self.write(&[
            SETUP_BUCKET,
            SET_BUCKET,
            index,
            id,
            (memory_slot >> 8) as u8,
            memory_slot as u8,
            memory_slot_count as u8,
            (memory_slot_count >> 8) as u8,
            1,
        ])
    }

    /// Send the write start to given bucket.
    pub fn write_start_bucket(&self, index: u8) -> Result<()> {
        self.write(&[WRITE_SETUP, WRITE_START, index])
    }

    /// Send the write finish to given bucket.
    pub fn write_finish_bucket(&self, index: u8) -> Result<()> {
        self.write(&[WRITE_SETUP, WRITE_FINISH, index])
    }

    /// Send the bulk data info for the given mode.
    pub fn send_bulk_data_info(&self, mode: u8) -> Result<()> {
        // Fill with 12fa01e8abcdef987654321 (magic numbers) and then mode,
        // couple of 0x00 and then more magic.
        self.write_bulk(&[
            0x12, 0xfa, 0x01, 0xe8, 0xab, 0xcd, 0xef, 0x98, 0x76, 0x54, 0x32, 0x10, mode, 0x00,
            0x00, 0x00, 0x00, 0x40, 0x96,
        ])
    }
}

impl Drop for NZXTDevice<'_> {
    /// Upon dropping NZXTDevice, ensure all interfaces are released and the device is reset.
    fn drop(&mut self) {
        #[cfg(not(target_os = "windows"))]
        self.handle
            .release_interface(0)
            .expect("Error releasing interface 0 for NZXTDevice.");

        self.handle
            .release_interface(1)
            .expect("Error releasing interface 1 for NZXTDevice.");

        self.handle
            .reset()
            .expect("Error resetting the NZXTDevice.");
    }
}

/// Parse the returned data bytes from the device into a firmware version.
fn parse_firmware_info(data: &[u8]) -> String {
    let major = data[0x11];
    let minor = data[0x12];
    let patch = data[0x13];

    format!("version {}.{}.{}", major, minor, patch)
}

/// Parse the response bytes into a device status.
fn parse_status(data: &[u8]) -> Result<DeviceStatus> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_mocks() -> MockNZXTDeviceHandle {
        let mut mock_nzxt_device_handle = MockNZXTDeviceHandle::new();

        mock_nzxt_device_handle
            .expect_claim_interface()
            .returning(|_| Ok(()));

        mock_nzxt_device_handle
            .expect_release_interface()
            .returning(|_| Ok(()));

        #[cfg(not(target_os = "windows"))]
        mock_nzxt_device_handle
            .expect_set_auto_detach_kernel_driver()
            .returning(|_| Ok(()));

        mock_nzxt_device_handle.expect_reset().returning(|| Ok(()));

        mock_nzxt_device_handle
    }

    #[test]
    fn test_new_device_success() {
        let mut mock_nzxt_device_handle = setup_mocks();

        // Mock the successful read of the device info.
        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, _, _| Ok(READ_LENGTH as usize));

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90);
        assert!(mock_nzxt_device.is_ok());
    }

    #[test]
    fn test_new_device_fail() {
        let mut mock_nzxt_device_handle = setup_mocks();

        // Mock the failure to read from device handle.
        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, _, _| Err(eyre!("Mock error")));

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90);
        assert!(mock_nzxt_device.is_err());
    }

    #[test]
    fn test_get_status() {
        let mut mock_nzxt_device_handle = setup_mocks();

        mock_nzxt_device_handle
            .expect_write_interrupt()
            .returning(|_, data, _| Ok(data.len() as usize));

        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, data, _| {
                // Successful response taken from real device.
                let response: [u8; READ_LENGTH] = [
                    117, 1, 57, 0, 42, 0, 24, 81, 57, 48, 51, 54, 50, 56, 1, 30, 9, 58, 9, 80, 80,
                    1, 2, 193, 6, 80, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ];
                data.copy_from_slice(&response);
                Ok(READ_LENGTH as usize)
            });

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90);
        assert!(mock_nzxt_device.is_ok());

        let mock_nzxt_device = mock_nzxt_device.unwrap();
        let status = mock_nzxt_device.get_status();
        assert!(status.is_ok());

        let status = status.unwrap();
        assert_eq!(status.temp, 30);
        assert_eq!(status.pump_duty, 80);
        assert_eq!(status.pump_rpm, 2362);
        assert_eq!(status.fan_duty, 80);
        assert_eq!(status.fan_rpm, 1729);
    }

    #[test]
    fn test_parse_firmware_version() {
        let data: [u8; READ_LENGTH] = [
            17, 1, 57, 0, 42, 0, 24, 81, 57, 48, 51, 54, 50, 56, 8, 48, 1, 5, 7, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let firmware_version = parse_firmware_info(&data);

        assert_eq!(firmware_version, "version 5.7.0");
    }

    #[test]
    fn test_parse_status() {
        // Successful response taken from real device.
        let response: [u8; READ_LENGTH] = [
            117, 1, 57, 0, 42, 0, 24, 81, 57, 48, 51, 54, 50, 56, 1, 30, 9, 58, 9, 80, 80, 1, 2,
            193, 6, 80, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let status = parse_status(&response);

        assert!(status.is_ok());

        let status = status.unwrap();

        assert_eq!(status.temp, 30);
        assert_eq!(status.pump_duty, 80);
        assert_eq!(status.pump_rpm, 2362);
        assert_eq!(status.fan_duty, 80);
        assert_eq!(status.fan_rpm, 1729);
    }

    #[test]
    fn test_set_brightness() {
        let mut mock_nzxt_device_handle = setup_mocks();

        // Mock the successful read of the device info.
        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, _, _| Ok(READ_LENGTH as usize));

        // Mock the successful write to device, expecting only 2 writes to occur.
        mock_nzxt_device_handle
            .expect_write_interrupt()
            .times(2)
            .returning(|_, data, _| Ok(data.len() as usize));

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90).unwrap();

        // Both the next two assertions should be fine.
        let result = mock_nzxt_device.set_brightness(100);
        assert!(result.is_ok());

        let result = mock_nzxt_device.set_brightness(0);
        assert!(result.is_ok());

        // Should fail, given value is over 100.
        let result = mock_nzxt_device.set_brightness(101);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_pump_duty() {
        let mut mock_nzxt_device_handle = setup_mocks();

        // Mock the successful read of the device info.
        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, _, _| Ok(READ_LENGTH as usize));

        // Mock the successful write to device, expecting only 2 writes to occur.
        mock_nzxt_device_handle
            .expect_write_interrupt()
            .times(2)
            .returning(|_, data, _| Ok(data.len() as usize));

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90).unwrap();

        let result = mock_nzxt_device.set_pump_duty(100);
        assert!(result.is_ok());

        let result = mock_nzxt_device.set_pump_duty(20);
        assert!(result.is_ok());

        let result = mock_nzxt_device.set_pump_duty(19);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_visual_mode() {
        let mut mock_nzxt_device_handle = setup_mocks();

        // Mock the successful read of the device info.
        mock_nzxt_device_handle
            .expect_read_interrupt()
            .returning(|_, _, _| Ok(READ_LENGTH as usize));

        // Mock the successful write to device, expecting only 1 write to occur.
        mock_nzxt_device_handle
            .expect_write_interrupt()
            .times(1)
            .returning(|_, data, _| Ok(data.len() as usize));

        let mock_nzxt_device = NZXTDevice::new(&mut mock_nzxt_device_handle, 90).unwrap();

        let result = mock_nzxt_device.set_visual_mode(0, 0);

        assert!(result.is_ok());
    }
}
