use common::ascii_frame::AsciiFrame;
use std::error::Error;
use std::time::{Duration, Instant};

/// Test patterns for local development
pub enum PatternType {
    Checkerboard,
    MovingLine,
}

/// Factory for "fake" frames to test locally.
pub struct MockFrameGenerator {
    /// width of mock ASCII frame
    w: usize,
    /// height of mock ASCII frame
    h: usize,
    /// counter to determine how ASCII frame should look temporally
    /// (i.e. when to alter characters)
    frame_counter: usize,
    /// determine current time
    last_frame_time: Instant,
    /// how long to wait to create a new frame (effectively FPS)
    frame_delay: Duration,
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

        let frame_delay = Duration::from_millis((1000 / fps) as u64);

        Ok(MockFrameGenerator {
            w,
            h,
            frame_counter: 0,
            last_frame_time: Instant::now(),
            frame_delay,
            pattern_type,
        })
    }

    /// Generate a mock frame
    pub fn generate_frame(&mut self) -> Result<AsciiFrame, Box<dyn Error>> {
        let elapsed = self.last_frame_time.elapsed();
        if elapsed < self.frame_delay {
            std::thread::sleep(self.frame_delay - elapsed);
        }
        self.last_frame_time = Instant::now();

        let mut frame = AsciiFrame::new(self.w, self.h, ' ')?;

        match self.pattern_type {
            PatternType::Checkerboard => self.generate_checkerboard(&mut frame),
            PatternType::MovingLine => self.generate_moving_line(&mut frame),
        }

        self.frame_counter += 1;

        Ok(frame)
    }

    /// Create a checkerboard pattern in the mock frame
    fn generate_checkerboard(&self, frame: &mut AsciiFrame) {
        let chars = ['.', '#'];

        for y in 0..self.h {
            for x in 0..self.w {
                let pattern_offset = (self.frame_counter / 5) % 2;
                let is_odd = (x + y) % 2;
                let i = (is_odd + pattern_offset) % 2;

                frame.set_char(x, y, chars[i]);
            }
        }
    }

    /// Create a moving line pattern in the mock frame
    fn generate_moving_line(&self, frame: &mut AsciiFrame) {
        let line_pos = self.frame_counter % frame.h;

        for y in 0..self.h {
            for x in 0..self.w {
                if y == line_pos {
                    frame.set_char(x, y, '=');
                } else {
                    frame.set_char(x, y, ' ');
                }
            }
        }
    }
}
