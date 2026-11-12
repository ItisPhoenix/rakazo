use chrono::{Datelike, Local, Timelike};
use std::{fs, io::Read, path::PathBuf};
use watchos_platform::{Clock, Display, DisplayConfig, LocalTime, PlatformError, Storage};

pub struct SystemClock;
impl Clock for SystemClock {
    fn local_time(&self) -> Result<LocalTime, PlatformError> {
        let now = Local::now();
        Ok(LocalTime {
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
            millisecond: now.timestamp_subsec_millis() as u16,
            year: now.year(),
            month: now.month() as u8,
            day: now.day() as u8,
            weekday: now.weekday().num_days_from_monday() as u8,
        })
    }
}

pub struct FileStorage(pub PathBuf);
impl Storage for FileStorage {
    fn load(&self, bytes: &mut [u8]) -> Result<usize, PlatformError> {
        let mut f = match fs::File::open(&self.0) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(PlatformError::Io),
        };
        if f.metadata().map_err(|_| PlatformError::Io)?.len() > bytes.len() as u64 {
            return Err(PlatformError::BufferTooSmall);
        }
        let mut count = 0;
        loop {
            let n = f.read(&mut bytes[count..]).map_err(|_| PlatformError::Io)?;
            if n == 0 {
                break;
            }
            count += n;
            if count == bytes.len() {
                let mut extra = [0];
                if f.read(&mut extra).map_err(|_| PlatformError::Io)? != 0 {
                    return Err(PlatformError::BufferTooSmall);
                }
                break;
            }
        }
        Ok(count)
    }
    fn save(&self, bytes: &[u8]) -> Result<(), PlatformError> {
        use std::io::Write;
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent).map_err(|_| PlatformError::Io)?;
        }
        let temporary = self.0.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = fs::File::create(&temporary).map_err(|_| PlatformError::Io)?;
        file.write_all(bytes).map_err(|_| PlatformError::Io)?;
        file.sync_all().map_err(|_| PlatformError::Io)?;
        drop(file);
        fs::rename(temporary, &self.0).map_err(|_| PlatformError::Io)
    }
}

pub struct PreviewDisplay(pub DisplayConfig);
impl Display for PreviewDisplay {
    fn config(&self) -> DisplayConfig {
        self.0
    }
    fn set_brightness(&mut self, percent: u8) -> Result<(), PlatformError> {
        if (20..=100).contains(&percent) {
            Ok(())
        } else {
            Err(PlatformError::InvalidData)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_replace_and_bounds() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/storage-test")
            .join(std::process::id().to_string())
            .join("settings.json");
        let storage = FileStorage(path.clone());
        let mut buffer = [0; 8];
        assert_eq!(storage.load(&mut buffer), Ok(0));
        storage.save(b"first").unwrap();
        storage.save(b"second").unwrap();
        assert_eq!(storage.load(&mut buffer), Ok(6));
        assert_eq!(&buffer[..6], b"second");
        assert_eq!(
            storage.load(&mut [0; 2]),
            Err(PlatformError::BufferTooSmall)
        );
        fs::remove_file(path).unwrap();
    }
}
