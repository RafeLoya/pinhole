use crate::config::PinholeConfig;
use crate::ffmpeg;
use crate::image_frame::ImageFrame;
use arc_swap::ArcSwap;
use ffmpeg_sidecar::child::FfmpegChild;
use std::error::Error;
use std::io::{BufReader, Read};
use std::process::ChildStdout;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Amount of bytes used per pixel in the RGB24 color format
const DEFAULT_BYTES_PER_PIXEL: usize = 3;

/// Frame data with sequence tracking for duplicate detection
struct FrameData {
    buffer: Vec<u8>,
    sequence: u64,
}

/// Spawns FFmpeg as a child process, reads the video frames
/// and captures it into an `ImageFrame`
pub struct Camera {
    /// Requested image width
    w: usize,
    /// Requested image height
    h: usize,
    /// FFmpeg child process, this component actually feeds the images
    /// to the program
    ffmpeg_proc: FfmpegChild,
    /// Reader, reads output frames from the FFmpeg child process
    frame_reader: Option<BufReader<ChildStdout>>,
    /// Intermediate buffer between FFmpeg child process and ImageFrame data.
    /// Used only by the non-frame-dropping `capture_frame` path.
    #[allow(dead_code)]
    frame_buffer: Vec<u8>,
    /// Background thread for continuous frame reading (frame dropping mode)
    reader_thread: Option<JoinHandle<()>>,
    /// Latest frame data shared with background thread (lock-free)
    latest_frame: Option<Arc<ArcSwap<FrameData>>>,
    /// Flag to stop the reader thread
    running: Option<Arc<Mutex<bool>>>,
    /// Last sequence number seen (for duplicate detection)
    last_sequence: u64,
}

impl Camera {
    /// Create a new Camera from configuration
    pub fn from_config(config: &PinholeConfig) -> Result<Self, Box<dyn Error>> {
        let (w, h) = match config.video.source.r#type.as_str() {
            "webcam" => (
                config.video.source.webcam.width,
                config.video.source.webcam.height,
            ),
            "screen" => (
                config.video.source.screen.width,
                config.video.source.screen.height,
            ),
            "file" => {
                // for files, need to parse dimensions from the stream
                // for now, using a default size - this could be improved
                (640, 480)
            }
            _ => (640, 480),
        };

        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        let mut ffmpeg_proc = ffmpeg::setup_from_config(config)?;

        let stdout = ffmpeg_proc
            .take_stdout()
            .ok_or("failed to get ffmpeg stdout")?;

        let bytes_per_pixel = DEFAULT_BYTES_PER_PIXEL;
        let buffer_size = w * h * bytes_per_pixel;

        Ok(Camera {
            w,
            h,
            ffmpeg_proc,
            frame_reader: Some(BufReader::with_capacity(buffer_size, stdout)),
            frame_buffer: vec![0u8; buffer_size],
            reader_thread: None,
            latest_frame: None,
            running: None,
            last_sequence: 0,
        })
    }

    /// Reads a frame provided by the camera into the provided `ImageFrame`.
    ///
    /// Currently unused since both solo and network modes read via the
    /// frame-dropping path; retained in case the network test surfaces a need
    /// for in-order reads.
    #[allow(dead_code)]
    pub fn capture_frame(&mut self, frame: &mut ImageFrame) -> Result<(), Box<dyn Error>> {
        if frame.w != self.w || frame.h != self.h {
            return Err(format!(
                "frame dimensions ({}x{}) do not match camera dimensions ({}x{})",
                frame.w, frame.h, self.w, self.h
            )
            .into());
        }

        let frame_reader = self
            .frame_reader
            .as_mut()
            .ok_or("frame reader not available (background mode active?)")?;

        // read in the frame
        if let Err(e) = frame_reader.read_exact(&mut self.frame_buffer) {
            return Err(format!("failed to read camera frame: {}", e).into());
        }

        if self.frame_buffer.len() != frame.buffer().len() {
            return Err(format!(
                "buffer size not consistent between camera ({}) and frame ({})",
                self.frame_buffer.len(),
                frame.buffer().len()
            )
            .into());
        }

        // copy the frame into the provided ImageFrame
        frame.buffer_mut().copy_from_slice(&self.frame_buffer);

        Ok(())
    }

    /// Enable frame-dropping mode: spawns a background thread that continuously
    /// reads frames from FFmpeg, keeping only the latest one. This prevents
    /// buffering lag by always providing the freshest available frame.
    /// Uses lock-free atomic swaps for zero-contention frame access.
    pub fn enable_frame_dropping(&mut self) -> Result<(), Box<dyn Error>> {
        if self.reader_thread.is_some() {
            return Err("frame dropping already enabled".into());
        }

        let mut frame_reader = self
            .frame_reader
            .take()
            .ok_or("frame reader already taken")?;

        let buffer_size = self.w * self.h * DEFAULT_BYTES_PER_PIXEL;
        let latest_frame = Arc::new(ArcSwap::from_pointee(FrameData {
            buffer: vec![0u8; buffer_size],
            sequence: 0,
        }));
        let latest_frame_clone = Arc::clone(&latest_frame);

        let running = Arc::new(Mutex::new(true));
        let running_clone = Arc::clone(&running);

        // spawn background thread to continuously read frames
        let reader_thread = thread::spawn(move || {
            let mut temp_buffer = vec![0u8; buffer_size];
            let mut sequence = 0u64;

            while *running_clone.lock().unwrap() {
                // read next frame from FFmpeg (blocking)
                match frame_reader.read_exact(&mut temp_buffer) {
                    Ok(_) => {
                        sequence += 1;
                        // atomically swap in new frame data (lock-free)
                        let new_frame = Arc::new(FrameData {
                            buffer: temp_buffer.clone(),
                            sequence,
                        });
                        latest_frame_clone.store(new_frame);
                    }
                    Err(_) => {
                        // FFmpeg stopped or error - exit thread
                        break;
                    }
                }
            }
        });

        self.latest_frame = Some(latest_frame);
        self.reader_thread = Some(reader_thread);
        self.running = Some(running);

        Ok(())
    }

    /// Capture the latest available frame (only works in frame-dropping mode).
    /// This always returns the freshest frame, dropping any buffered older frames.
    /// Returns Ok(true) if a new frame was captured, Ok(false) if duplicate frame.
    pub fn capture_latest_frame(&mut self, frame: &mut ImageFrame) -> Result<bool, Box<dyn Error>> {
        if frame.w != self.w || frame.h != self.h {
            return Err(format!(
                "frame dimensions ({}x{}) do not match camera dimensions ({}x{})",
                frame.w, frame.h, self.w, self.h
            )
            .into());
        }

        let latest_frame = self
            .latest_frame
            .as_ref()
            .ok_or("frame dropping not enabled - call enable_frame_dropping() first")?;

        // load the latest frame data (lock-free atomic operation)
        let frame_data = latest_frame.load_full();

        if frame_data.buffer.len() != frame.buffer().len() {
            return Err(format!(
                "buffer size not consistent between camera ({}) and frame ({})",
                frame_data.buffer.len(),
                frame.buffer().len()
            )
            .into());
        }

        // check if this is a duplicate frame
        let is_new_frame = frame_data.sequence != self.last_sequence;
        self.last_sequence = frame_data.sequence;

        // copy latest frame
        frame.buffer_mut().copy_from_slice(&frame_data.buffer);

        Ok(is_new_frame)
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // kill FFmpeg first, this unblocks any read_exact() calls
        if let Err(e) = self.ffmpeg_proc.kill() {
            eprintln!("failed to kill ffmpeg: {}", e);
        }

        // signal background thread to stop
        if let Some(running) = &self.running {
            *running.lock().unwrap() = false;
        }

        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
    }
}
