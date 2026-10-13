//! Rendered images checked by rules, not by matching stored pixels.
//!
//! Capture a frame however the program renders (an offscreen GPU readback, a
//! software rasterizer, a screenshot of a private display), then check what
//! must be true of any correct frame: nothing is clipped, the region that
//! should show text has contrast, the scene is neither black nor blown out.
//! Save the frame as an artifact for a person or agent to look at.

use crate::check::{Failure, fail};

/// An 8-bit RGBA image, row-major from the top-left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Frame {
    /// A frame from raw RGBA bytes.
    ///
    /// # Errors
    /// Fails unless `rgba` holds exactly `width * height * 4` bytes.
    pub fn from_rgba(width: usize, height: usize, rgba: Vec<u8>) -> Result<Self, Failure> {
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| fail("frame size overflows"))?;
        if rgba.len() != expected {
            return Err(fail(format!(
                "{width}x{height} RGBA needs {expected} bytes, got {}",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// A frame filled with one colour.
    ///
    /// # Errors
    /// Fails when the size overflows.
    pub fn filled(width: usize, height: usize, rgba: [u8; 4]) -> Result<Self, Failure> {
        let pixels = width
            .checked_mul(height)
            .ok_or_else(|| fail("frame size overflows"))?;
        Self::from_rgba(
            width,
            height,
            std::iter::repeat_n(rgba, pixels).flatten().collect(),
        )
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    fn offset(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        y.checked_mul(self.width)?.checked_add(x)?.checked_mul(4)
    }

    /// The pixel at `(x, y)`, or `None` outside the frame.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<[u8; 4]> {
        let start = self.offset(x, y)?;
        let bytes = self.rgba.get(start..start.checked_add(4)?)?;
        bytes.try_into().ok()
    }

    /// Set the pixel at `(x, y)`; `None` outside the frame.
    pub fn set_pixel(&mut self, x: usize, y: usize, rgba: [u8; 4]) -> Option<()> {
        let start = self.offset(x, y)?;
        let end = start.checked_add(4)?;
        self.rgba
            .get_mut(start..end)?
            .copy_from_slice_checked(&rgba)
    }

    /// The `width x height` region at `(x, y)`, or `None` if it leaves the frame.
    #[must_use]
    pub fn region(&self, x: usize, y: usize, width: usize, height: usize) -> Option<Self> {
        if x.checked_add(width)? > self.width || y.checked_add(height)? > self.height {
            return None;
        }
        let mut rgba = Vec::with_capacity(width.checked_mul(height)?.checked_mul(4)?);
        for row in y..y.checked_add(height)? {
            let start = self.offset(x, row)?;
            let end = start.checked_add(width.checked_mul(4)?)?;
            rgba.extend_from_slice(self.rgba.get(start..end)?);
        }
        Some(Self {
            width,
            height,
            rgba,
        })
    }

    /// Summary statistics, for rules like "the scene is lit".
    #[must_use]
    pub fn stats(&self) -> Stats {
        // Wrapping cannot occur: a frame holds at most usize::MAX / 4 pixels
        // and each adds at most 255 to a u64 total.
        let mut stats = Stats::default();
        let mut luma_total = 0_u64;
        for pixel in self.rgba.as_chunks::<4>().0 {
            let [red, green, blue, _] = *pixel;
            // Rec. 709 luma in 0..=255, in integer per-mille weights.
            let luma = (u64::from(red).wrapping_mul(213))
                .wrapping_add(u64::from(green).wrapping_mul(715))
                .wrapping_add(u64::from(blue).wrapping_mul(72))
                .checked_div(1000)
                .unwrap_or_default();
            luma_total = luma_total.wrapping_add(luma);
            stats.pixels = stats.pixels.wrapping_add(1);
            if luma < 16 {
                stats.dark = stats.dark.wrapping_add(1);
            } else if luma > 239 {
                stats.bright = stats.bright.wrapping_add(1);
            }
        }
        stats.mean_luma = luma_total.checked_div(stats.pixels).unwrap_or_default();
        stats
    }

    /// The frame as a binary PPM (alpha dropped): viewable everywhere, no
    /// image crate needed.
    #[must_use]
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for pixel in self.rgba.as_chunks::<4>().0 {
            let [red, green, blue, _] = *pixel;
            out.extend([red, green, blue]);
        }
        out
    }
}

/// Copying that reports a length mismatch instead of panicking.
trait CopyChecked {
    fn copy_from_slice_checked(&mut self, source: &[u8]) -> Option<()>;
}

impl CopyChecked for [u8] {
    fn copy_from_slice_checked(&mut self, source: &[u8]) -> Option<()> {
        if self.len() != source.len() {
            return None;
        }
        for (target, byte) in self.iter_mut().zip(source) {
            *target = *byte;
        }
        Some(())
    }
}

/// Luma statistics of a frame (luma 0..=255).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Pixels counted.
    pub pixels: u64,
    /// Mean luma.
    pub mean_luma: u64,
    /// Pixels with luma below 16.
    pub dark: u64,
    /// Pixels with luma above 239.
    pub bright: u64,
}

impl Stats {
    /// The share of dark pixels in per mille.
    #[must_use]
    // At most usize::MAX / 4 pixels, so * 1000 fits in a u64.
    pub fn dark_per_mille(&self) -> u64 {
        self.dark
            .wrapping_mul(1000)
            .checked_div(self.pixels)
            .unwrap_or_default()
    }

    /// The share of bright pixels in per mille.
    #[must_use]
    pub fn bright_per_mille(&self) -> u64 {
        self.bright
            .wrapping_mul(1000)
            .checked_div(self.pixels)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;
    use crate::check::{Failure, ensure, ensure_eq, fail};

    #[test]
    fn stats_describe_the_scene() -> Result<(), Failure> {
        let mut frame = Frame::filled(10, 10, [0, 0, 0, 255])?;
        for x in 0..10 {
            frame
                .set_pixel(x, 0, [255, 255, 255, 255])
                .ok_or_else(|| fail("pixel outside"))?;
        }
        let stats = frame.stats();
        ensure_eq(
            &(stats.dark_per_mille(), stats.bright_per_mille()),
            &(900, 100),
            "dark and bright shares",
        )
    }

    #[test]
    fn region_and_pixel_reject_out_of_bounds() -> Result<(), Failure> {
        let frame = Frame::filled(4, 3, [1, 2, 3, 4])?;
        ensure(
            frame.pixel(4, 0).is_none() && frame.region(2, 1, 3, 1).is_none(),
            "out of bounds accepted",
        )?;
        let region = frame
            .region(1, 1, 2, 2)
            .ok_or_else(|| fail("in-bounds region refused"))?;
        ensure_eq(&region.pixel(1, 1), &Some([1, 2, 3, 4]), "region pixel")
    }

    #[test]
    fn ppm_has_a_header_and_three_bytes_per_pixel() -> Result<(), Failure> {
        let ppm = Frame::filled(2, 2, [9, 8, 7, 6])?.to_ppm();
        ensure(ppm.starts_with(b"P6\n2 2\n255\n"), "bad PPM header")?;
        ensure_eq(&ppm.len(), &(11 + 12), "PPM length")
    }

    #[test]
    fn luma_thresholds_and_edges() -> Result<(), Failure> {
        let gray =
            |value| Frame::filled(3, 2, [value, value, value, 255]).map(|frame| frame.stats());
        ensure_eq(&(gray(15)?.dark, gray(16)?.dark), &(6, 0), "dark threshold")?;
        ensure_eq(
            &(gray(239)?.bright, gray(240)?.bright),
            &(0, 6),
            "bright threshold",
        )?;
        let frame = Frame::filled(3, 2, [0; 4])?;
        ensure_eq(&(frame.width(), frame.height()), &(3, 2), "size")?;
        ensure(
            frame.region(1, 0, 2, 2).is_some(),
            "a region touching the edges was refused",
        )?;
        ensure(
            frame.region(0, 1, 3, 2).is_none(),
            "a region below the frame was accepted",
        )
    }
}
