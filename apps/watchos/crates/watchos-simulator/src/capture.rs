use slint::ComponentHandle;
use std::{
    cell::Cell,
    fs::File,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use watchos_ui::WatchWindow;

const MOTION_FRAMES: usize = 24;
const FRAME_DELAY_MS: u32 = 100;

/// Stream snapshots of the running Slint window after Home has finished booting.
pub fn start_motion(ui: &WatchWindow, directory: PathBuf, failed: Rc<Cell<bool>>) {
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(1200), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        match MotionCapture::new(directory, ui.get_logical_size()) {
            Ok(capture) => capture_frame(weak, capture, failed),
            Err(error) => finish_motion(Err(error), &failed),
        }
    });
}

struct MotionCapture {
    directory: PathBuf,
    name: String,
    encoder: image::codecs::gif::GifEncoder<File>,
    output: File,
    first: Option<image::RgbaImage>,
    last: Option<image::RgbaImage>,
    phases: Vec<f32>,
    frame_delays: Vec<u32>,
    previous_capture: Option<Instant>,
    frames: Vec<(image::RgbaImage, u32)>,
    nonidentical_frames: usize,
}

impl MotionCapture {
    fn new(directory: PathBuf, size: i32) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&directory)?;
        let name = format!("hybrid-motion-{size}");
        let output = File::create(directory.join(format!("{name}.gif")))?;
        let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(output.try_clone()?, 10);
        encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
        Ok(Self {
            directory,
            name,
            encoder,
            output,
            first: None,
            last: None,
            phases: Vec::with_capacity(MOTION_FRAMES),
            frame_delays: Vec::with_capacity(MOTION_FRAMES),
            previous_capture: None,
            frames: Vec::with_capacity(MOTION_FRAMES),
            nonidentical_frames: 0,
        })
    }

    fn frame(&mut self, ui: &WatchWindow) -> Result<(), Box<dyn std::error::Error>> {
        let captured_at = Instant::now();
        let delay = self
            .previous_capture
            .map(|previous| {
                captured_at
                    .duration_since(previous)
                    .as_millis()
                    .clamp(1, u128::from(u32::MAX)) as u32
            })
            .unwrap_or(FRAME_DELAY_MS);
        self.previous_capture = Some(captured_at);
        self.frame_delays.push(delay);
        let snapshot = ui.window().take_snapshot()?;
        let pixels = image::RgbaImage::from_raw(
            snapshot.width(),
            snapshot.height(),
            snapshot.as_bytes().to_vec(),
        )
        .ok_or("Invalid motion snapshot dimensions")?;
        if self.last.as_ref().is_some_and(|last| last != &pixels) {
            self.nonidentical_frames += 1;
        }
        if self.first.is_none() {
            self.first = Some(pixels.clone());
        }
        self.frames.push((pixels.clone(), delay));
        self.last = Some(pixels);
        self.phases.push(ui.get_motion_phase());
        Ok(())
    }

    fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Bounded diagnostic capture: encode after sampling so quantization cannot stall UI motion.
        for (pixels, delay) in self.frames {
            self.encoder.encode_frame(image::Frame::from_parts(
                pixels,
                0,
                0,
                image::Delay::from_numer_denom_ms(delay, 1),
            ))?;
        }
        drop(self.encoder);
        self.output.sync_all()?;
        let first = self.first.ok_or("No motion frames captured")?;
        let last = self.last.ok_or("No final motion frame captured")?;
        first.save(self.directory.join(format!("{}-first.png", self.name)))?;
        last.save(self.directory.join(format!("{}-last.png", self.name)))?;
        std::fs::write(
            self.directory.join(format!("{}.txt", self.name)),
            format!(
                "source=Slint native window snapshots\nframes={}\nminimum_capture_interval_ms={FRAME_DELAY_MS}\nwarmup_ms=1200\nwidth={}\nheight={}\nnonidentical_frames={}\nphase_values={:?}\nactual_frame_delays_ms={:?}\n",
                self.phases.len(),
                first.width(),
                first.height(),
                self.nonidentical_frames,
                self.phases,
                self.frame_delays
            ),
        )?;
        verify_motion(self.nonidentical_frames, &self.phases)?;
        println!(
            "MOTION CAPTURE PASS: {} frames, {} changed frames: {}",
            self.phases.len(),
            self.nonidentical_frames,
            self.directory.join(format!("{}.gif", self.name)).display()
        );
        Ok(())
    }
}

fn capture_frame(
    weak: slint::Weak<WatchWindow>,
    mut capture: MotionCapture,
    failed: Rc<Cell<bool>>,
) {
    let Some(ui) = weak.upgrade() else {
        return;
    };
    if let Err(error) = capture.frame(&ui) {
        finish_motion(Err(error), &failed);
    } else if capture.phases.len() == MOTION_FRAMES {
        finish_motion(capture.finish(), &failed);
    } else {
        slint::Timer::single_shot(
            Duration::from_millis(u64::from(FRAME_DELAY_MS)),
            move || {
                capture_frame(weak, capture, failed);
            },
        );
    }
}

fn finish_motion(result: Result<(), Box<dyn std::error::Error>>, failed: &Cell<bool>) {
    if let Err(error) = result {
        failed.set(true);
        eprintln!("MOTION CAPTURE FAIL: {error}");
    }
    if let Err(error) = slint::quit_event_loop() {
        failed.set(true);
        eprintln!("Cannot stop motion capture: {error}");
    }
}

fn verify_motion(nonidentical_frames: usize, phases: &[f32]) -> Result<(), &'static str> {
    if nonidentical_frames == 0 {
        return Err("All captured motion frames are identical");
    }
    if phases.iter().any(|phase| !phase.is_finite())
        || !phases.windows(2).any(|pair| pair[0] != pair[1])
    {
        return Err("Motion phase did not advance to different finite values");
    }
    Ok(())
}

/// Capture the native software renderer, not a separately drawn mockup.
pub fn save(
    ui: &WatchWindow,
    directory: &Path,
    name: &str,
    size: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    let snapshot = ui.window().take_snapshot()?;
    let pixels = image::RgbaImage::from_raw(
        snapshot.width(),
        snapshot.height(),
        snapshot.as_bytes().to_vec(),
    )
    .ok_or("Invalid snapshot dimensions")?;
    pixels.save(directory.join(format!("{name}.png")))?;
    let scale = ui.window().scale_factor();
    let diameter = (f32::from(size) * scale).round() as u32;
    let left = (pixels
        .width()
        .checked_sub(diameter)
        .ok_or("Snapshot too small")?)
        / 2;
    let top = (56. * scale).round() as u32;
    if top + diameter > pixels.height() {
        return Err("Dial exceeds snapshot".into());
    }
    image::imageops::crop_imm(&pixels, left, top, diameter, diameter)
        .to_image()
        .save(directory.join(format!("{name}-dial.png")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::verify_motion;

    #[test]
    fn motion_requires_pixel_and_phase_changes() {
        assert!(verify_motion(0, &[0.0, 0.1]).is_err());
        assert!(verify_motion(1, &[0.1, 0.1]).is_err());
        assert!(verify_motion(1, &[0.0, f32::NAN]).is_err());
        assert!(verify_motion(1, &[0.0, 0.1]).is_ok());
    }
}
