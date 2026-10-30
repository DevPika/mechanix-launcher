//! System D-Bus services shared by the launcher's screens. Each module is
//! safe to add more than once; only the first install takes effect.
pub mod bluetooth;
pub mod display;
pub mod network_manager;
pub mod power;
pub mod upower;

pub use bluetooth::{
    BluetoothModule, BluetoothState, BluetoothStateChanged, BluetoothStatus, ToggleBluetooth,
};
pub use display::{
    BrightnessChanged, DisplayModule, MAX_BRIGHTNESS, MIN_BRIGHTNESS, SetBrightness,
};
pub use network_manager::{APUpdate, NetworkManagerModule, SetWifiEnabled, StateUpdate, WifiState};
pub use power::{PowerModule, PowerOff, Restart};
pub use upower::{BatteryPercentageChanged, UpowerModule};
