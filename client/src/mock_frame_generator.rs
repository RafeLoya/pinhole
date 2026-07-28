use common::text_frame::TextFrame;
use common::frame_pixel::FramePixel;
use std::error::Error;

/// Test patterns for local development
#[derive(Clone)]
pub enum PatternType {
    Checkerboard,
    MovingLine,
}

/// Factory for "fake" frames to test locally.
///
/// Frame pacing is the caller's responsibility (see `Client::pace_frame`); this
/// generator only produces frame content.
pub struct MockFrameGenerator {
    /// width of mock ASCII frame
    w: usize,
    /// height of mock ASCII frame
    h: usize,
    /// counter to determine how ASCII frame should look temporally
    /// (i.e. when to alter characters)
    frame_counter: usize,
    /// pattern to generate
    pattern_type: PatternType,
}

impl MockFrameGenerator {
    pub fn new(
        w: usize,
        h: usize,
        fps: u32,
        pattern_type: PatternType,
    ) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 || fps < 1 {
            return Err("failed to create mock frame generator".into());
        }

        Ok(MockFrameGenerator {
            w,
            h,
            frame_counter: 0,
            pattern_type,
        })
    }

    /// Generate a mock frame
    pub fn generate_frame(&mut self) -> Result<TextFrame, Box<dyn Error>> {
        let mut frame = TextFrame::new(self.w, self.h, ' ')?;

        match self.pattern_type {
            PatternType::Checkerboard => self.generate_checkerboard(&mut frame),
            PatternType::MovingLine => self.generate_moving_line(&mut frame),
        }

        self.frame_counter += 1;

        Ok(frame)
    }

    /// Create a checkerboard pattern in the mock frame
    fn generate_checkerboard(&self, frame: &mut TextFrame) {
        // alternate between intensity indices 3 and 7
        let indices = [3, 7];

        for y in 0..self.h {
            for x in 0..self.w {
                let pattern_offset = (self.frame_counter / 5) % 2;
                let is_odd = (x + y) % 2;
                let i = (is_odd + pattern_offset) % 2;

                frame.set_pixel(x, y, FramePixel::intensity(indices[i]));
            }
        }
    }

    /// Create a moving line pattern in the mock frame
    fn generate_moving_line(&self, frame: &mut TextFrame) {
        let line_pos = self.frame_counter % frame.h;

        for y in 0..self.h {
            for x in 0..self.w {
                if y == line_pos {
                    // use horizontal edge with index 2 (thick line)
                    frame.set_pixel(x, y, FramePixel::horizontal_edge(2));
                } else {
                    // use intensity 0 (space)
                    frame.set_pixel(x, y, FramePixel::intensity(0));
                }
            }
        }
    }
}
