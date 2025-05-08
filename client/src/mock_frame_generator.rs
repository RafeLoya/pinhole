use common::ascii_frame::AsciiFrame;
use std::error::Error;
use std::time::{Duration, Instant};

pub enum PatternType {
    Checkerboard,
    MovingLine,
}

pub struct MockFrameGenerator {
    w: usize,
    h: usize,
    frame_counter: usize,
    last_frame_time: Instant,
    frame_delay: Duration,
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

        let frame_delay = Duration::from_micros((1000000.0 / fps as f64) as u64);

        Ok(MockFrameGenerator {
            w,
            h,
            frame_counter: 0,
            last_frame_time: Instant::now(),
            frame_delay,
            pattern_type,
        })
    }

    pub fn generate_frame(&mut self) -> Result<AsciiFrame, Box<dyn Error>> {
        let elapsed = self.last_frame_time.elapsed();
        if elapsed < self.frame_delay {
            //std::thread::sleep(self.frame_delay - elapsed);
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
