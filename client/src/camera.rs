use crate::config::PinholeConfig;
use crate::ffmpeg;
use crate::image_frame::ImageFrame;
use ffmpeg_sidecar::child::FfmpegChild;
use std::error::Error;
use std::io::{BufReader, Read};
use std::process::ChildStdout;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Amount of bytes used per pixel in the RGB24 color format
const DEFAULT_BYTES_PER_PIXEL: usize = 3;

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
    /// Intermediate buffer between FFmpeg child process and ImageFrame data
    frame_buffer: Vec<u8>,
    /// Background thread for continuous frame reading (frame dropping mode)
    reader_thread: Option<JoinHandle<()>>,
    /// Latest frame buffer shared with background thread
    latest_frame: Option<Arc<Mutex<Vec<u8>>>>,
    /// Flag to stop the reader thread
    running: Option<Arc<Mutex<bool>>>,
}

impl Camera {
    /// Create a new Camera using the default FFmpeg setup (deprecated)
    #[deprecated(note = "Use from_config instead")]
    pub fn new(w: usize, h: usize) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        let mut ffmpeg_proc = ffmpeg::setup_default()?;

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
        })
    }

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
                // For files, need to parse dimensions from the stream
                // For now, using a default size - this could be improved
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
        })
    }

    /// Reads a frame provided by the camera into the provided `ImageFrame`
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
    pub fn enable_frame_dropping(&mut self) -> Result<(), Box<dyn Error>> {
        if self.reader_thread.is_some() {
            return Err("frame dropping already enabled".into());
        }

        let mut frame_reader = self
            .frame_reader
            .take()
            .ok_or("frame reader already taken")?;

        let buffer_size = self.w * self.h * DEFAULT_BYTES_PER_PIXEL;
        let latest_frame = Arc::new(Mutex::new(vec![0u8; buffer_size]));
        let latest_frame_clone = Arc::clone(&latest_frame);

        let running = Arc::new(Mutex::new(true));
        let running_clone = Arc::clone(&running);

        // Spawn background thread to continuously read frames
        let reader_thread = thread::spawn(move || {
            let mut temp_buffer = vec![0u8; buffer_size];

            while *running_clone.lock().unwrap() {
                // Read next frame from FFmpeg (blocking)
                match frame_reader.read_exact(&mut temp_buffer) {
                    Ok(_) => {
                        // Update the latest frame (drop old one)
                        let mut latest = latest_frame_clone.lock().unwrap();
                        latest.copy_from_slice(&temp_buffer);
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
    pub fn capture_latest_frame(&mut self, frame: &mut ImageFrame) -> Result<(), Box<dyn Error>> {
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

        // get the latest frame from the background thread
        let frame_data = latest_frame.lock().unwrap();

        if frame_data.len() != frame.buffer().len() {
            return Err(format!(
                "buffer size not consistent between camera ({}) and frame ({})",
                frame_data.len(),
                frame.buffer().len()
            )
            .into());
        }

        // copy latest frame
        frame.buffer_mut().copy_from_slice(&frame_data);

        Ok(())
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // stop background thread if running
        if let Some(running) = &self.running {
            *running.lock().unwrap() = false;
        }

        // wait for thread to finish
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }

        // kill FFmpeg when Camera is dropped
        if let Err(e) = self.ffmpeg_proc.kill() {
            eprintln!("failed to kill ffmpeg: {}", e);
        }
    }
}
