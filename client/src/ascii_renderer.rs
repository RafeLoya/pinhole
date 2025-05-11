use common::ascii_frame::AsciiFrame;
use std::error::Error;
use std::io;
use std::io::Write;
use tokio::time::Instant;
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
    pub fn clear_screen() -> Result<(), Box<dyn Error>> {
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush()?;
        Ok(())
    }

    /// With an `AsciiFrame`, output any ASCII characters that changed from
    /// `prev_frame` to the screen, and record these changes into
    /// `prev_frame`
    pub fn render(&mut self, frame: &AsciiFrame) -> Result<(), Box<dyn Error>> {
        // did frame size change?
        let start = Instant::now();

        if frame.w != self.prev_w
            || frame.h != self.prev_h
            || self.prev_frame.len() != frame.w * frame.h
        {
            self.prev_frame = vec![' '; frame.w * frame.h];
            self.prev_w = frame.w;
            self.prev_h = frame.h;

            Self::clear_screen()?;
        }

        //print!("\x1B[1;1H{:?}", frame.chars().to_vec());

        for y in 0..frame.h {
            for x in 0..frame.w {
                let i = y * frame.w + x;

                if i < frame.chars().len()
                    && i < self.prev_frame.len()
                    && frame.chars()[i] != self.prev_frame[i]
                {
                    // ANSI escape code sequence, move cursor to specified
                    // row & column & change character
                    print!("\x1B[{};{}H{}", y + 1, x + 1, frame.chars()[i]);
                    self.prev_frame[i] = frame.chars()[i];
                }
            }
        }

        io::stdout().flush()?;

        let end = Instant::now();

        Ok(())
    }

    pub fn process_datagram(&mut self, datagram: &[u8]) -> Result<AsciiFrame, Box<dyn Error>> {
        if datagram.len() < 16 {
            return Err("frame too small (size header too small)".into());
        }

        let mut w_bytes = [0u8; 8];
        w_bytes.copy_from_slice(&datagram[0..8]);
        let w = usize::from_be_bytes(w_bytes);

        let mut h_bytes = [0u8; 8];
        h_bytes.copy_from_slice(&datagram[8..16]);
        let h = usize::from_be_bytes(h_bytes);

        AsciiFrame::from_bytes(w, h, &datagram[16..])

        // if w * h + 16 > datagram.len() {
        //     return Err(format!(
        //         "incomplete frame: expected {} bytes but got {}",
        //         w * h,
        //         datagram.len() - 16
        //     )
        //     .into());
        // }
        //
        // // TODO: review this
        // AsciiFrame::from_bytes(w, h, &datagram[16..16 + w * h])
    }

    pub fn serialize_frame(frame: &AsciiFrame) -> Vec<u8> {
        //let mut bytes = Vec::with_capacity(16 + frame.w * frame.h);
        let mut bytes = Vec::with_capacity(16 + frame.w * frame.h * 4);
        bytes.extend_from_slice(&frame.w.to_be_bytes());
        bytes.extend_from_slice(&frame.h.to_be_bytes());
        bytes.extend_from_slice(&frame.bytes());

        bytes
    }
}
