//! The network backend: NetworkManager in production, `FakeNetwork` in tests.

use crate::error::NetworkError;
use crate::model::NetworkSnapshot;

pub trait Network: Send {
    fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError>;
    fn set_wifi_enabled(&mut self, on: bool) -> Result<(), NetworkError>;
    fn set_airplane(&mut self, on: bool) -> Result<(), NetworkError>;
    /// Activates a network. A password, when given, is handed to the backend
    /// inside the activation call and never kept.
    fn connect(&mut self, id: &str, password: Option<&str>) -> Result<(), NetworkError>;
    fn disconnect(&mut self, id: &str) -> Result<(), NetworkError>;
    fn forget(&mut self, id: &str) -> Result<(), NetworkError>;
    fn set_vpn_active(&mut self, id: &str, on: bool) -> Result<(), NetworkError>;
}
