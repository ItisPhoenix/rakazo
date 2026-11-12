#![no_std]

use watchos_protocol::ConnectionState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformError {
    Unavailable,
    Io,
    InvalidData,
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayConfig {
    pub width: u16,
    pub height: u16,
}
impl DisplayConfig {
    pub const DEFAULT: Self = Self {
        width: 240,
        height: 240,
    };
    pub fn is_valid(self) -> bool {
        (240..=800).contains(&self.width) && self.width == self.height
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
}

pub trait Clock {
    fn local_time(&self) -> Result<LocalTime, PlatformError>;
}
pub trait Storage {
    fn load(&self, bytes: &mut [u8]) -> Result<usize, PlatformError>;
    fn save(&self, bytes: &[u8]) -> Result<(), PlatformError>;
}
pub trait Display {
    fn config(&self) -> DisplayConfig;
    fn set_brightness(&mut self, percent: u8) -> Result<(), PlatformError>;
}
#[derive(Clone, Copy, Debug)]
pub enum InputEvent {
    Touch { x: u16, y: u16 },
    Back,
    Home,
}
pub trait Input {
    fn poll(&mut self) -> Option<InputEvent>;
}
pub trait Vibration {
    fn vibrate(&mut self, milliseconds: u16) -> Result<(), PlatformError>;
}
pub trait Connectivity {
    fn state(&self) -> ConnectionState;
    fn send(&mut self, frame: &[u8]) -> Result<(), PlatformError>;
    fn receive(&mut self, frame: &mut [u8]) -> Result<usize, PlatformError>;
}
pub trait DataProvider {
    fn telemetry(&self) -> watchos_core::Telemetry;
}

pub struct UnavailableHardware;
impl DataProvider for UnavailableHardware {
    fn telemetry(&self) -> watchos_core::Telemetry {
        watchos_core::Telemetry::default()
    }
}
impl Vibration for UnavailableHardware {
    fn vibrate(&mut self, _: u16) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable)
    }
}
impl Connectivity for UnavailableHardware {
    fn state(&self) -> ConnectionState {
        ConnectionState::Unavailable
    }
    fn send(&mut self, _: &[u8]) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable)
    }
    fn receive(&mut self, _: &mut [u8]) -> Result<usize, PlatformError> {
        Err(PlatformError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_is_not_a_success() {
        let mut h = UnavailableHardware;
        assert_eq!(h.state(), ConnectionState::Unavailable);
        assert_eq!(h.send(&[]), Err(PlatformError::Unavailable));
        assert_eq!(h.vibrate(10), Err(PlatformError::Unavailable));
        assert_eq!(h.telemetry(), watchos_core::Telemetry::default());
        assert!(DisplayConfig::DEFAULT.is_valid());
        assert!(
            !DisplayConfig {
                width: 0,
                height: 0
            }
            .is_valid()
        );
        assert!(
            !DisplayConfig {
                width: 320,
                height: 240
            }
            .is_valid()
        );
    }
}
