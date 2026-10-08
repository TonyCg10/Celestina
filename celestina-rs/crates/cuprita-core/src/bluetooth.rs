//! The Bluetooth backend: BlueZ in production, `FakeBluetooth` in tests.

use crate::error::BluetoothError;
use crate::model::BluetoothSnapshot;

pub trait Bluetooth: Send {
    fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError>;
    fn set_powered(&mut self, on: bool) -> Result<(), BluetoothError>;
    fn set_discovering(&mut self, on: bool) -> Result<(), BluetoothError>;
    fn pair(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn connect(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn disconnect(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn forget(&mut self, address: &str) -> Result<(), BluetoothError>;
}
