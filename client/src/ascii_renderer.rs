use std::error::Error;
use std::io;
use std::io::Write;
use common::ascii_frame::AsciiFrame;

// TODO: changing window / frame sizes during runtime

/// Outputs ASCII frame data to `stdout`
pub struct AsciiRenderer {
    /// used to reduce terminal flickering and
    /// (to later be used) for changing window sizes
    prev_frame: Vec<char>,
    /// width of previous `AsciiFrame`
    prev_w: usize,
    /// height of previous `AsciiFrame`
    prev_h: usize,
}

impl AsciiRenderer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Self::clear_screen()?;

        Ok(AsciiRenderer {
            prev_frame: Vec::new(),
            prev_w: 0,
            prev_h: 0,
        })
    }

    /// Prints an ANSI escape code sequence that clears the screen
    /// and positions the cursor in the top-left corner (1, 1).
    /// `stdout` is then flushed to print to the terminal as soon as possible.
    fn clear_screen() -> Result<(), Box<dyn Error>> {
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush()?;
        Ok(())
    }

    /// With an `AsciiFrame`, output any ASCII characters that changed from
    /// `prev_frame` to the screen, and record these changes into
    /// `prev_frame`
    pub fn render(&mut self, frame: &AsciiFrame) -> Result<(), Box<dyn Error>> {
        // did frame size change?
        if frame.w != self.prev_w
            || frame.h != self.prev_h
            || self.prev_frame.len() != frame.w * frame.h {

            self.prev_frame = vec![' '; frame.w * frame.h];
            self.prev_w = frame.w;
            self.prev_h = frame.h;

            Self::clear_screen()?;
        }

        for y in 0..frame.h {
            for x in 0..frame.w {
                let i = y * frame.w + x;

                if i < frame.chars().len()
                    && i < self.prev_frame.len()
                    && frame.chars()[i] != self.prev_frame[i] {

                    // ANSI escape code sequence, move cursor to specified
                    // row & column & change character
                    print!("\x1B[{};{}H{}", y+1, x+1, frame.chars()[i]);
                    self.prev_frame[i] = frame.chars()[i];
                }
            }
        }

        io::stdout().flush()?;

        Ok(())
    }
}