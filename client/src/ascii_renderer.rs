use common::ascii_frame::AsciiFrame;
use std::error::Error;
use std::io;
use std::io::Write;
use itoa;

/// Outputs ASCII frame data to `stdout`
pub struct AsciiRenderer {
    /// Used to reduce terminal flickering and
    /// (to later be used) for changing window sizes
    prev_frame: Vec<char>,
    /// Width of previous `AsciiFrame`
    prev_w: usize,
    /// Height of previous `AsciiFrame`
    prev_h: usize,
    /// Reusable buffer for batched terminal output
    output_buffer: Vec<u8>,
}

impl AsciiRenderer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Self::clear_screen()?;
        Self::hide_cursor()?;

        Ok(AsciiRenderer {
            prev_frame: Vec::new(),
            prev_w: 0,
            prev_h: 0,
            // pre-allocate buffer for ~120x40 display with ANSI escape codes
            // each char change: ESC[row;colHchar (worst case, 9 + 4 = 13 bytes)
            // 120x40 = 4,800 chars * 13 bytes = 62,400 bytes for full frame
            output_buffer: Vec::with_capacity(80_000),
        })
    }

    /// Hides the terminal cursor
    fn hide_cursor() -> Result<(), Box<dyn Error>> {
        print!("\x1B[?25l");
        io::stdout().flush()?;
        Ok(())
    }

    /// Shows the terminal cursor
    fn show_cursor() -> Result<(), Box<dyn Error>> {
        print!("\x1B[?25h");
        io::stdout().flush()?;
        Ok(())
    }

    /// Prints an ANSI escape code sequence that clears the screen
    /// and positions the cursor in the top-left corner (1, 1).
    /// `stdout` is then flushed to print to the terminal as soon as possible.
    fn clear_screen() -> Result<(), Box<dyn Error>> {
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush()?;
        Ok(())
    }

    /// Prepares the output buffer without writing to stdout.
    ///
    /// Separate function for benchmarking the rendering logic without I/O overhead.
    /// Returns the size of the prepared buffer.
    pub fn prepare_buffer(&mut self, frame: &AsciiFrame) -> Result<usize, Box<dyn Error>> {
        // did frame size change?
        if frame.w != self.prev_w
            || frame.h != self.prev_h
            || self.prev_frame.len() != frame.w * frame.h
        {
            self.prev_frame = vec![' '; frame.w * frame.h];
            self.prev_w = frame.w;
            self.prev_h = frame.h;
        }

        // clear buffer but keep capacity
        self.output_buffer.clear();

        for y in 0..frame.h {
            for x in 0..frame.w {
                let i = y * frame.w + x;

                if i < frame.chars().len()
                    && i < self.prev_frame.len()
                    && frame.chars()[i] != self.prev_frame[i]
                {
                    let ch = frame.chars()[i];

                    // write ANSI escape code sequence: ESC [ row ; col H char
                    // ESC [
                    self.output_buffer.push(0x1B);
                    self.output_buffer.push(b'[');

                    // row (y + 1)
                    let mut buf = itoa::Buffer::new();
                    self.output_buffer.extend_from_slice(buf.format(y + 1).as_bytes());

                    // ;
                    self.output_buffer.push(b';');

                    // col (x + 1)
                    let mut buf = itoa::Buffer::new();
                    self.output_buffer.extend_from_slice(buf.format(x + 1).as_bytes());

                    // H
                    self.output_buffer.push(b'H');

                    // encode char to UTF-8 and append
                    let mut char_buf = [0u8; 4];
                    let char_str = ch.encode_utf8(&mut char_buf);
                    self.output_buffer.extend_from_slice(char_str.as_bytes());

                    self.prev_frame[i] = ch;
                }
            }
        }

        Ok(self.output_buffer.len())
    }

    /// With an `AsciiFrame`, output any ASCII characters that changed from
    /// `prev_frame` to the screen, and record these changes into
    /// `prev_frame`
    pub fn render(&mut self, frame: &AsciiFrame) -> Result<(), Box<dyn Error>> {
        // check if frame size changed and clear screen if needed
        let size_changed = frame.w != self.prev_w
            || frame.h != self.prev_h
            || self.prev_frame.len() != frame.w * frame.h;

        if size_changed {
            Self::clear_screen()?;
        }

        // prepare the buffer (this handles size changes internally)
        let buffer_size = self.prepare_buffer(frame)?;

        // write entire buffer to stdout in one syscall
        if buffer_size > 0 {
            io::stdout().write_all(&self.output_buffer)?;
            io::stdout().flush()?;
        }

        Ok(())
    }

    /// Deserializes an array of bytes into an `AsciiFrame`, if it is valid
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
    }

    /// Serializes an array of bytes into an `AsciiFrame`
    pub fn serialize_frame(frame: &AsciiFrame) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + frame.w * frame.h * 4);
        bytes.extend_from_slice(&frame.w.to_be_bytes());
        bytes.extend_from_slice(&frame.h.to_be_bytes());
        bytes.extend_from_slice(&frame.bytes());

        bytes
    }

    /// Write a status message below the rendered frame
    ///
    /// Positions the cursor at row (frame_height + 1) and writes the message.
    /// This prevents the message from being overwritten by the frame rendering.
    pub fn write_status_message(&self, message: &str) -> Result<(), Box<dyn Error>> {
        // position cursor below the frame (row = prev_h + 1, col = 1)
        print!("\x1B[{};1H", self.prev_h + 1);
        // clear the line to remove any previous message
        print!("\x1B[2K");
        print!("{}", message);
        io::stdout().flush()?;
        Ok(())
    }
}

impl Drop for AsciiRenderer {
    fn drop(&mut self) {
        // clear screen and restore cursor visibility when renderer is dropped
        let _ = Self::clear_screen();
        let _ = Self::show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_single_char() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(5, 3, ' ').unwrap();

        // set a single character
        frame.set_char(2, 1, 'X');

        // first render should output the character
        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify prev_frame was updated
        assert_eq!(renderer.prev_frame[1 * 5 + 2], 'X');
    }

    #[test]
    fn test_render_no_changes() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let frame = AsciiFrame::new(5, 3, ' ').unwrap();

        // render once
        renderer.render(&frame).unwrap();

        // buffer should be empty on second identical render
        renderer.output_buffer.clear();
        renderer.render(&frame).unwrap();
        assert_eq!(renderer.output_buffer.len(), 0);
    }

    #[test]
    fn test_render_multiple_changes() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(10, 5, ' ').unwrap();

        // set multiple characters
        frame.set_char(0, 0, 'A');
        frame.set_char(9, 4, 'B');
        frame.set_char(5, 2, 'C');

        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify all characters were updated
        assert_eq!(renderer.prev_frame[0], 'A');
        assert_eq!(renderer.prev_frame[4 * 10 + 9], 'B');
        assert_eq!(renderer.prev_frame[2 * 10 + 5], 'C');
    }

    #[test]
    fn test_render_utf8_characters() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(5, 3, ' ').unwrap();

        // test with various UTF-8 characters
        frame.set_char(0, 0, '■');  // block element (3 bytes)
        frame.set_char(1, 0, '━');  // box drawing (3 bytes)
        frame.set_char(2, 0, '│');  // box drawing (3 bytes)
        frame.set_char(3, 0, '@');  // ASCII (1 byte)

        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify output buffer contains UTF-8 encoded data
        assert!(renderer.output_buffer.len() > 0);
    }

    #[test]
    fn test_render_frame_size_change() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let frame1 = AsciiFrame::new(10, 5, 'A').unwrap();

        renderer.render(&frame1).unwrap();
        assert_eq!(renderer.prev_w, 10);
        assert_eq!(renderer.prev_h, 5);

        // change frame size
        let frame2 = AsciiFrame::new(20, 10, 'B').unwrap();
        renderer.render(&frame2).unwrap();

        assert_eq!(renderer.prev_w, 20);
        assert_eq!(renderer.prev_h, 10);
        assert_eq!(renderer.prev_frame.len(), 200);
    }

    #[test]
    fn test_output_buffer_reuse() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(10, 5, ' ').unwrap();

        frame.set_char(0, 0, 'X');
        renderer.render(&frame).unwrap();

        let capacity_after_first = renderer.output_buffer.capacity();

        // render again with different change
        frame.set_char(1, 1, 'Y');
        renderer.render(&frame).unwrap();

        // capacity should remain the same (buffer reused)
        assert_eq!(renderer.output_buffer.capacity(), capacity_after_first);
    }
}
