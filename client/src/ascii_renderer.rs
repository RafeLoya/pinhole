use common::ascii_frame::AsciiFrame;
use common::frame_encoding::{FrameType, PixelChange};
use common::frame_pixel::FramePixel;
use std::error::Error;
use std::io;
use std::io::Write;
use itoa;

// box-drawing characters for TUI border
const BOX_TOP_LEFT: char = '┌';
const BOX_TOP_RIGHT: char = '┐';
const BOX_BOTTOM_LEFT: char = '└';
const BOX_BOTTOM_RIGHT: char = '┘';
const BOX_HORIZONTAL: char = '─';
const BOX_VERTICAL: char = '│';

/// Layout configuration for TUI chrome (border, status, debug pane)
#[derive(Clone)]
pub struct TuiLayout {
    /// Width of the video area
    pub video_width: usize,
    /// Height of the video area
    pub video_height: usize,
    /// Whether border is visible
    pub border_visible: bool,
    /// Whether debug pane is visible
    pub debug_visible: bool,
}

impl TuiLayout {
    /// Create a new TUI layout with the given video dimensions.
    pub fn new(video_width: usize, video_height: usize) -> Self {
        Self {
            video_width,
            video_height,
            border_visible: true,
            debug_visible: false,
        }
    }

    /// Row offset for video content (1 if border visible, 0 otherwise).
    #[inline]
    pub fn video_row_offset(&self) -> usize {
        if self.border_visible { 1 } else { 0 }
    }

    /// Column offset for video content (1 if border visible, 0 otherwise).
    #[inline]
    pub fn video_col_offset(&self) -> usize {
        if self.border_visible { 1 } else { 0 }
    }

    /// Row number for status line (1-indexed for ANSI).
    pub fn status_row(&self) -> usize {
        if self.border_visible {
            // border top (1) + video height + border bottom (1) + 1
            self.video_height + 3
        } else {
            self.video_height + 1
        }
    }

    /// First row of debug pane (1-indexed for ANSI).
    pub fn debug_start_row(&self) -> usize {
        self.status_row() + 1
    }

    /// Total width including border.
    pub fn total_width(&self) -> usize {
        if self.border_visible {
            self.video_width + 2
        } else {
            self.video_width
        }
    }

    /// Total height including border, status, and debug.
    pub fn total_height(&self) -> usize {
        let mut height = self.video_height;
        if self.border_visible {
            height += 2; // top and bottom border
        }
        height += 1; // status line
        if self.debug_visible {
            height += 2; // debug pane (2 lines)
        }
        height
    }
}

/// Performance statistics for debug pane display.
#[derive(Clone, Default)]
pub struct PerformanceStats {
    /// Actual frames per second
    pub fps: f32,
    /// Time to process last frame in milliseconds
    pub frame_time_ms: f32,
    /// Ratio of diff frames to total frames (0.0 - 1.0)
    pub compression_ratio: f32,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Size of last frame in bytes
    pub last_frame_size: usize,
}

/// Format bytes as human-readable string (KB, MB, GB).
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f64 / 1_000.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Outputs ASCII frame data to `stdout`
pub struct AsciiRenderer {
    /// Used to reduce terminal flickering and
    /// (to later be used) for changing window sizes
    prev_frame: Vec<FramePixel>,
    /// Width of previous `AsciiFrame`
    prev_w: usize,
    /// Height of previous `AsciiFrame`
    prev_h: usize,
    /// Reusable buffer for batched terminal output
    output_buffer: Vec<u8>,
    /// Character mappings for rendering
    intensity_chars: Vec<char>,
    horizontal_line_chars: Vec<char>,
    vertical_line_chars: Vec<char>,
    forward_diagonal_chars: Vec<char>,
    back_diagonal_chars: Vec<char>,
    /// Current assembled frame (for applying diffs)
    current_frame: Option<AsciiFrame>,
    /// TUI layout configuration
    layout: TuiLayout,
    /// Whether border has been rendered (avoids redundant draws)
    border_rendered: bool,
}

impl AsciiRenderer {
    /// Create a new renderer with character mappings from configuration.
    pub fn new_with_chars(
        intensity_chars: Vec<char>,
        horizontal_line_chars: Vec<char>,
        vertical_line_chars: Vec<char>,
        forward_diagonal_chars: Vec<char>,
        back_diagonal_chars: Vec<char>,
    ) -> Result<Self, Box<dyn Error>> {
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
            intensity_chars,
            horizontal_line_chars,
            vertical_line_chars,
            forward_diagonal_chars,
            back_diagonal_chars,
            current_frame: None,
            layout: TuiLayout::new(0, 0), // will be updated on first render
            border_rendered: false,
        })
    }

    /// Create a new renderer with TUI layout configuration.
    pub fn new_with_layout(
        intensity_chars: Vec<char>,
        horizontal_line_chars: Vec<char>,
        vertical_line_chars: Vec<char>,
        forward_diagonal_chars: Vec<char>,
        back_diagonal_chars: Vec<char>,
        layout: TuiLayout,
    ) -> Result<Self, Box<dyn Error>> {
        Self::clear_screen()?;
        Self::hide_cursor()?;

        Ok(AsciiRenderer {
            prev_frame: Vec::new(),
            prev_w: 0,
            prev_h: 0,
            output_buffer: Vec::with_capacity(80_000),
            intensity_chars,
            horizontal_line_chars,
            vertical_line_chars,
            forward_diagonal_chars,
            back_diagonal_chars,
            current_frame: None,
            layout,
            border_rendered: false,
        })
    }

    /// Create a new renderer with default character mappings
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_chars(
            " .:coPO?@■".chars().collect(),
            "-━═".chars().collect(),
            "|│┃".chars().collect(),
            "/╱⟋".chars().collect(),
            "\\╲⟍".chars().collect(),
        )
    }

    /// Map a FramePixel to its character representation
    ///
    /// Performance-critical: called for every changed pixel in hot rendering loop.
    /// Uses unchecked indexing since FramePixel indices are validated at creation.
    #[inline(always)]
    fn pixel_to_char(&self, pixel: FramePixel) -> char {
        let idx = pixel.index() as usize;
        let category = pixel.category();

        // SAFETY: FramePixel creation validates indices are within bounds.
        // intensity: max 16, edges: max 8. Config validation enforces these limits.
        unsafe {
            match category {
                0 => *self.intensity_chars.get_unchecked(idx.min(self.intensity_chars.len() - 1)),
                1 => *self.horizontal_line_chars.get_unchecked(idx.min(self.horizontal_line_chars.len() - 1)),
                2 => *self.vertical_line_chars.get_unchecked(idx.min(self.vertical_line_chars.len() - 1)),
                3 => *self.forward_diagonal_chars.get_unchecked(idx.min(self.forward_diagonal_chars.len() - 1)),
                4 => *self.back_diagonal_chars.get_unchecked(idx.min(self.back_diagonal_chars.len() - 1)),
                _ => ' ',
            }
        }
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

    /// Render the TUI border around the video area.
    pub fn render_border(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.layout.border_visible {
            return Ok(());
        }

        let w = self.layout.video_width;
        let h = self.layout.video_height;

        // top border: ┌───...───┐
        print!("\x1B[1;1H{}", BOX_TOP_LEFT);
        for _ in 0..w {
            print!("{}", BOX_HORIZONTAL);
        }
        print!("{}", BOX_TOP_RIGHT);

        // side borders for each video row
        for row in 0..h {
            let screen_row = row + 2; // 1-indexed, after top border
            // left border
            print!("\x1B[{};1H{}", screen_row, BOX_VERTICAL);
            // right border
            print!("\x1B[{};{}H{}", screen_row, w + 2, BOX_VERTICAL);
        }

        // bottom border: └───...───┘
        print!("\x1B[{};1H{}", h + 2, BOX_BOTTOM_LEFT);
        for _ in 0..w {
            print!("{}", BOX_HORIZONTAL);
        }
        print!("{}", BOX_BOTTOM_RIGHT);

        io::stdout().flush()?;
        self.border_rendered = true;
        Ok(())
    }

    /// Toggle border visibility.
    pub fn toggle_border(&mut self) {
        self.layout.border_visible = !self.layout.border_visible;
        self.border_rendered = false;
        // clear screen to remove old border or prepare for new one
        let _ = Self::clear_screen();
        // force full frame redraw
        self.prev_frame.clear();
        self.prev_w = 0;
        self.prev_h = 0;
    }

    /// Toggle debug pane visibility.
    pub fn toggle_debug(&mut self) {
        self.layout.debug_visible = !self.layout.debug_visible;
        if !self.layout.debug_visible {
            // clear debug area
            let debug_row = self.layout.debug_start_row();
            print!("\x1B[{};1H\x1B[2K", debug_row);
            print!("\x1B[{};1H\x1B[2K", debug_row + 1);
            let _ = io::stdout().flush();
        }
    }

    /// Check if border is visible.
    pub fn is_border_visible(&self) -> bool {
        self.layout.border_visible
    }

    /// Check if debug pane is visible.
    pub fn is_debug_visible(&self) -> bool {
        self.layout.debug_visible
    }

    /// Get a mutable reference to the layout.
    pub fn layout_mut(&mut self) -> &mut TuiLayout {
        &mut self.layout
    }

    /// Render the debug pane with performance statistics.
    pub fn render_debug_pane(&self, stats: &PerformanceStats) -> Result<(), Box<dyn Error>> {
        if !self.layout.debug_visible {
            return Ok(());
        }

        let row = self.layout.debug_start_row();

        // line 1: FPS and frame time
        print!("\x1B[{};1H\x1B[2K", row);
        print!(
            "FPS: {:5.1}  Frame: {:5.1}ms  Compression: {:5.1}%",
            stats.fps,
            stats.frame_time_ms,
            stats.compression_ratio * 100.0
        );

        // line 2: bytes sent/received
        print!("\x1B[{};1H\x1B[2K", row + 1);
        print!(
            "Sent: {}  Recv: {}  Last: {} bytes",
            format_bytes(stats.bytes_sent),
            format_bytes(stats.bytes_received),
            stats.last_frame_size
        );

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
            self.prev_frame = vec![FramePixel::default(); frame.w * frame.h];
            self.prev_w = frame.w;
            self.prev_h = frame.h;
            // update layout dimensions
            self.layout.video_width = frame.w;
            self.layout.video_height = frame.h;
            // border needs to be redrawn with new dimensions
            self.border_rendered = false;
        }

        // clear buffer but keep capacity
        self.output_buffer.clear();

        // get coordinate offsets for border
        let row_offset = self.layout.video_row_offset();
        let col_offset = self.layout.video_col_offset();

        for y in 0..frame.h {
            for x in 0..frame.w {
                let i = y * frame.w + x;

                if i < frame.pixels().len()
                    && i < self.prev_frame.len()
                    && frame.pixels()[i] != self.prev_frame[i]
                {
                    let pixel = frame.pixels()[i];
                    let ch = self.pixel_to_char(pixel);

                    // write ANSI escape code sequence: ESC [ row ; col H char
                    // ESC [
                    self.output_buffer.push(0x1B);
                    self.output_buffer.push(b'[');

                    // row (y + 1 + row_offset for border)
                    let mut buf = itoa::Buffer::new();
                    self.output_buffer.extend_from_slice(buf.format(y + 1 + row_offset).as_bytes());

                    // ;
                    self.output_buffer.push(b';');

                    // col (x + 1 + col_offset for border)
                    let mut buf = itoa::Buffer::new();
                    self.output_buffer.extend_from_slice(buf.format(x + 1 + col_offset).as_bytes());

                    // H
                    self.output_buffer.push(b'H');

                    // encode char to UTF-8 and append
                    let mut char_buf = [0u8; 4];
                    let char_str = ch.encode_utf8(&mut char_buf);
                    self.output_buffer.extend_from_slice(char_str.as_bytes());

                    self.prev_frame[i] = pixel;
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
            self.border_rendered = false;
        }

        // render border if needed
        if self.layout.border_visible && !self.border_rendered {
            self.render_border()?;
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

    /// Deserializes datagram into an `AsciiFrame`, supporting both Full and Diff frames
    pub fn process_datagram(&mut self, datagram: &[u8]) -> Result<AsciiFrame, Box<dyn Error>> {
        if datagram.is_empty() {
            return Err("empty datagram".into());
        }

        let frame_type = FrameType::from_byte(datagram[0])
            .ok_or("invalid frame type")?;

        let result = match frame_type {
            FrameType::Full => self.process_full_frame(&datagram[1..]),
            FrameType::Diff => self.process_diff_frame(&datagram[1..]),
        };

        // on successful Full frame, clear renderer state to force full redraw
        if result.is_ok() && frame_type == FrameType::Full {
            // force full redraw by clearing prev_frame
            self.prev_frame.clear();
            self.prev_w = 0;
            self.prev_h = 0;
        }

        result
    }

    /// Process a full frame
    fn process_full_frame(&mut self, data: &[u8]) -> Result<AsciiFrame, Box<dyn Error>> {
        if data.len() < 16 {
            return Err("full frame too small".into());
        }

        let mut w_bytes = [0u8; 8];
        w_bytes.copy_from_slice(&data[0..8]);
        let w = usize::from_be_bytes(w_bytes);

        let mut h_bytes = [0u8; 8];
        h_bytes.copy_from_slice(&data[8..16]);
        let h = usize::from_be_bytes(h_bytes);

        let expected_size = 16 + (w * h);
        if data.len() != expected_size {
            return Err(format!(
                "full frame size mismatch: expected {} bytes ({}x{} + header), got {}",
                expected_size, w, h, data.len()
            ).into());
        }

        let frame = AsciiFrame::from_bytes(w, h, &data[16..])?;

        // if dimensions changed, clear the screen to avoid artifacts
        if let Some(ref curr) = self.current_frame {
            if curr.w != frame.w || curr.h != frame.h {
                let _ = Self::clear_screen();
            }
        }

        self.current_frame = Some(frame.clone());
        Ok(frame)
    }

    /// Process a diff frame and apply changes to current frame
    fn process_diff_frame(&mut self, data: &[u8]) -> Result<AsciiFrame, Box<dyn Error>> {
        if data.len() < 4 {
            return Err("diff frame too small".into());
        }

        // ensure we have a current frame to apply diffs to
        let current = self.current_frame.as_mut()
            .ok_or("cannot apply diff without initial full frame")?;

        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&data[0..4]);
        let change_count = u32::from_be_bytes(count_bytes) as usize;

        let expected_size = 4 + (change_count * 3);
        if data.len() != expected_size {
            return Err(format!(
                "diff frame size mismatch: expected {} bytes ({} changes), got {}",
                expected_size, change_count, data.len()
            ).into());
        }

        // apply changes
        let mut offset = 4;
        for _ in 0..change_count {
            if offset + 3 > data.len() {
                return Err("diff frame truncated".into());
            }

            let change_bytes: [u8; 3] = [data[offset], data[offset + 1], data[offset + 2]];
            let change = PixelChange::from_bytes(&change_bytes);

            if (change.index as usize) < current.pixels().len() {
                current.pixels_mut()[change.index as usize] = change.pixel;
            }

            offset += 3;
        }

        Ok(current.clone())
    }

    /// Serializes an `AsciiFrame` into bytes (full frame, no compression)
    ///
    /// Deprecated: Use FrameSerializer for diff-based compression
    pub fn serialize_frame(frame: &AsciiFrame) -> Vec<u8> {
        // type (1 byte) + width (8 bytes) + height (8 bytes) + pixel data
        let mut bytes = Vec::with_capacity(17 + frame.w * frame.h);
        bytes.push(FrameType::Full.as_byte());
        bytes.extend_from_slice(&frame.w.to_be_bytes());
        bytes.extend_from_slice(&frame.h.to_be_bytes());
        bytes.extend_from_slice(&frame.bytes());

        bytes
    }

    /// Write a status message below the rendered frame.
    ///
    /// Positions the cursor at the status row (accounting for border) and writes the message.
    /// This prevents the message from being overwritten by the frame rendering.
    pub fn write_status_message(&self, message: &str) -> Result<(), Box<dyn Error>> {
        // position cursor at the status row
        let row = self.layout.status_row();
        print!("\x1B[{};1H", row);
        // clear the line to remove any previous message
        print!("\x1B[2K");
        print!("{}", message);
        io::stdout().flush()?;
        Ok(())
    }
}

impl Drop for AsciiRenderer {
    fn drop(&mut self) {
        // restore cursor visibility when renderer is dropped
        // note: screen clearing is handled by TerminalGuard's LeaveAlternateScreen
        let _ = Self::show_cursor();
    }
}

/// Stateful frame serializer that supports diff-based compression
pub struct FrameSerializer {
    /// Previous frame sent (for computing diffs)
    prev_frame: Option<AsciiFrame>,
    /// Reusable buffer for encoding changes
    changes_buffer: Vec<PixelChange>,
    /// Stats tracking
    pub total_frames: u64,
    pub full_frames: u64,
    pub diff_frames: u64,
    pub total_bytes: u64,
}

impl FrameSerializer {
    pub fn new() -> Self {
        Self {
            prev_frame: None,
            changes_buffer: Vec::with_capacity(1024),
            total_frames: 0,
            full_frames: 0,
            diff_frames: 0,
            total_bytes: 0,
        }
    }

    /// Serialize a frame with diff compression
    ///
    /// First frame is always Full. Subsequent frames are Diff if changes < 50% of pixels.
    pub fn serialize(&mut self, frame: &AsciiFrame) -> Vec<u8> {
        // first frame or dimension changed? send full frame
        let send_full = self.prev_frame.as_ref().map_or(true, |prev| {
            prev.w != frame.w || prev.h != frame.h
        });

        if send_full {
            let bytes = self.serialize_full(frame);
            self.prev_frame = Some(frame.clone());
            return bytes;
        }

        // compute diff
        self.changes_buffer.clear();
        let prev = self.prev_frame.as_ref().unwrap();

        for i in 0..frame.pixels().len() {
            if frame.pixels()[i] != prev.pixels()[i] {
                if i <= u16::MAX as usize {
                    self.changes_buffer.push(PixelChange::new(
                        i as u16,
                        frame.pixels()[i],
                    ));
                }
            }
        }

        // if more than 50% changed, send full frame (better compression)
        let change_ratio = self.changes_buffer.len() as f32 / frame.pixels().len() as f32;
        let bytes = if change_ratio > 0.5 {
            self.serialize_full(frame)
        } else {
            self.serialize_diff()
        };

        self.prev_frame = Some(frame.clone());
        bytes
    }

    /// Serialize as full frame
    fn serialize_full(&mut self, frame: &AsciiFrame) -> Vec<u8> {
        // type (1 byte) + width (8 bytes) + height (8 bytes) + all pixels
        let mut bytes = Vec::with_capacity(17 + frame.w * frame.h);
        bytes.push(FrameType::Full.as_byte());
        bytes.extend_from_slice(&frame.w.to_be_bytes());
        bytes.extend_from_slice(&frame.h.to_be_bytes());
        bytes.extend_from_slice(&frame.bytes());

        // update stats
        self.total_frames += 1;
        self.full_frames += 1;
        self.total_bytes += bytes.len() as u64;

        bytes
    }

    /// Serialize as diff frame
    fn serialize_diff(&mut self) -> Vec<u8> {
        // type (1 byte) + change count (4 bytes) + changes (3 bytes each)
        let mut bytes = Vec::with_capacity(5 + self.changes_buffer.len() * 3);
        bytes.push(FrameType::Diff.as_byte());
        bytes.extend_from_slice(&(self.changes_buffer.len() as u32).to_be_bytes());

        for change in &self.changes_buffer {
            bytes.extend_from_slice(&change.to_bytes());
        }

        // update stats
        self.total_frames += 1;
        self.diff_frames += 1;
        self.total_bytes += bytes.len() as u64;

        bytes
    }

    /// Reset serializer state (e.g., after connection reset)
    pub fn reset(&mut self) {
        self.prev_frame = None;
        self.changes_buffer.clear();
    }
}

impl Default for FrameSerializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_single_pixel() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(5, 3, ' ').unwrap();

        // set a single pixel with intensity
        let pixel = FramePixel::intensity(5);
        frame.pixels_mut()[1 * 5 + 2] = pixel;

        // first render should output the character
        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify prev_frame was updated
        assert_eq!(renderer.prev_frame[1 * 5 + 2], pixel);
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

        // set multiple pixels with different intensities
        let pixel_a = FramePixel::intensity(1);
        let pixel_b = FramePixel::intensity(2);
        let pixel_c = FramePixel::intensity(3);
        frame.pixels_mut()[0] = pixel_a;
        frame.pixels_mut()[4 * 10 + 9] = pixel_b;
        frame.pixels_mut()[2 * 10 + 5] = pixel_c;

        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify all pixels were updated in prev_frame
        assert_eq!(renderer.prev_frame[0], pixel_a);
        assert_eq!(renderer.prev_frame[4 * 10 + 9], pixel_b);
        assert_eq!(renderer.prev_frame[2 * 10 + 5], pixel_c);
    }

    #[test]
    fn test_render_edge_characters() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(5, 3, ' ').unwrap();

        // test with various edge types
        frame.pixels_mut()[0] = FramePixel::horizontal_edge(0);
        frame.pixels_mut()[1] = FramePixel::vertical_edge(0);
        frame.pixels_mut()[2] = FramePixel::forward_diagonal(0);
        frame.pixels_mut()[3] = FramePixel::back_diagonal(0);

        let result = renderer.render(&frame);
        assert!(result.is_ok());

        // verify output buffer contains data
        assert!(renderer.output_buffer.len() > 0);
    }

    #[test]
    fn test_render_frame_size_change() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let frame1 = AsciiFrame::new(10, 5, ' ').unwrap();

        renderer.render(&frame1).unwrap();
        assert_eq!(renderer.prev_w, 10);
        assert_eq!(renderer.prev_h, 5);

        // change frame size
        let frame2 = AsciiFrame::new(20, 10, ' ').unwrap();
        renderer.render(&frame2).unwrap();

        assert_eq!(renderer.prev_w, 20);
        assert_eq!(renderer.prev_h, 10);
        assert_eq!(renderer.prev_frame.len(), 200);
    }

    #[test]
    fn test_output_buffer_reuse() {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut frame = AsciiFrame::new(10, 5, ' ').unwrap();

        frame.pixels_mut()[0] = FramePixel::intensity(5);
        renderer.render(&frame).unwrap();

        let capacity_after_first = renderer.output_buffer.capacity();

        // render again with different change
        frame.pixels_mut()[11] = FramePixel::intensity(6);
        renderer.render(&frame).unwrap();

        // capacity should remain the same (buffer reused)
        assert_eq!(renderer.output_buffer.capacity(), capacity_after_first);
    }
}
