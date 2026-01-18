use crate::image_frame::ImageFrame;
use arc_swap::ArcSwap;
use crossbeam::channel::{self, Receiver, Sender};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;

// TODO: Look into Robert's Cross operator as potential alternative (if slow performance)
// TODO: Remove `.unwrap()`s in the future for error recovery
// TODO: Allow user to influence data members

/// The edge values for each pixel in a given ImageFrame
pub struct EdgeInfo {
    /// The strength / intensity of an edge, if it exists
    pub magnitude: Vec<f32>,
    /// The angle of an edge, if it exists
    pub angle: Vec<f32>,
    /// The width of the camera it will receive image frames from
    pub w: usize,
    /// The height of the camera it will receive image frames from
    pub h: usize,
}

/// Reusable buffers for edge detection processing.
/// These are owned by the processing thread and never shared.
struct ProcessingBuffers {
    /// Grayscale intensity map (w * h)
    intensity: Vec<f32>,
    /// Sobel gradient in x direction (w * h)
    gx: Vec<f32>,
    /// Sobel gradient in y direction (w * h)
    gy: Vec<f32>,
    /// Edge magnitude values (w * h)
    magnitude: Vec<f32>,
    /// Edge angle values (w * h)
    angle: Vec<f32>,
    /// Result buffer for non-maximum suppression (w * h)
    suppression_result: Vec<f32>,
    /// Dimensions these buffers were allocated for
    w: usize,
    h: usize,
}

impl ProcessingBuffers {
    /// Create new buffers pre-allocated to given dimensions
    fn new(w: usize, h: usize) -> Self {
        let size = w * h;
        Self {
            intensity: vec![0.0; size],
            gx: vec![0.0; size],
            gy: vec![0.0; size],
            magnitude: vec![0.0; size],
            angle: vec![0.0; size],
            suppression_result: vec![0.0; size],
            w,
            h,
        }
    }

    /// Resize buffers if dimensions changed (rare case)
    fn resize_if_needed(&mut self, w: usize, h: usize) {
        if self.w != w || self.h != h {
            let size = w * h;
            self.intensity.resize(size, 0.0);
            self.gx.resize(size, 0.0);
            self.gy.resize(size, 0.0);
            self.magnitude.resize(size, 0.0);
            self.angle.resize(size, 0.0);
            self.suppression_result.resize(size, 0.0);
            self.w = w;
            self.h = h;
        }
    }
}

/// Thread that processes given `ImageFrames` using our edge detection methods
/// and returns that information to apply it to the final `TextFrame`
pub struct EdgeDetector {
    /// The edge magnitudes and angles of the latest processed `ImageFrame`.
    /// Uses atomic swap for lock-free reads.
    edge_info: Arc<ArcSwap<EdgeInfo>>,
    /// Channel sender for submitting frames to the processing thread
    /// Wrapped in Option so we can close it before joining thread in Drop
    frame_sender: Option<Sender<ImageFrame>>,
    /// Channel receiver for the processing thread (taken in start())
    frame_receiver: Arc<Mutex<Option<Receiver<ImageFrame>>>>,
    /// Minimum gradient magnitude threshold.
    /// Operates from 0.0 to 255.0
    threshold: f32,
    /// Control flag, will terminate the edge detection thread when `false`
    running: Arc<Mutex<bool>>,
    /// JoinHandle for the processing thread
    thread_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl EdgeDetector {
    /// Default `threshold` value if none is provided
    pub const DEFAULT_EDGE_THRESHOLD: f32 = 20.0;

    pub fn new(w: usize, h: usize, threshold: f32) -> Self {
        let edge_info = Arc::new(ArcSwap::from_pointee(EdgeInfo {
            magnitude: vec![0.0; w * h],
            angle: vec![0.0; w * h],
            w,
            h,
        }));

        // create bounded channel with capacity 1 for frame submission
        let (frame_sender, frame_receiver) = channel::bounded(1);
        let running = Arc::new(Mutex::new(true));

        Self {
            edge_info,
            frame_sender: Some(frame_sender),
            frame_receiver: Arc::new(Mutex::new(Some(frame_receiver))),
            threshold,
            running,
            thread_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Launches the edge detection processing thread.
    ///
    /// This processing thread continuously receives frames from the channel,
    /// processes them with various algorithms to obtain edge information.
    /// Communication between threads is handled via a bounded channel.
    ///
    /// # Errors
    ///
    /// Returns an error if `start()` has already been called (receiver already taken).
    pub fn start(
        &self,
        cam_w: usize,
        cam_h: usize,
    ) -> Result<(), Box<dyn Error>> {
        let edge_info: Arc<ArcSwap<EdgeInfo>> = Arc::clone(&self.edge_info);
        let running = Arc::clone(&self.running);
        let threshold = self.threshold;

        // take the receiver (can only call start() once)
        let frame_receiver = self
            .frame_receiver
            .lock()
            .unwrap()
            .take()
            .ok_or("EdgeDetector already started")?;

        let handle = thread::spawn(move || {
            // create processing buffers once for this thread
            let mut buffers = ProcessingBuffers::new(cam_w, cam_h);

            loop {
                // block until frame arrives or channel disconnects
                match frame_receiver.recv() {
                    Ok(frame) => {
                        // check running flag before processing
                        if !*running.lock().unwrap() {
                            break;
                        }

                        // resize buffers if frame dimensions changed (rare case)
                        buffers.resize_if_needed(frame.w, frame.h);

                        // process frame using reusable buffers
                        if let Ok(()) = Self::process_frame(&frame, threshold, &mut buffers) {
                            // create new EdgeInfo and atomically swap it in
                            let new_info = Arc::new(EdgeInfo {
                                magnitude: buffers.magnitude.clone(),
                                angle: buffers.angle.clone(),
                                w: buffers.w,
                                h: buffers.h,
                            });
                            edge_info.store(new_info);
                        }
                    }
                    Err(_) => {
                        // channel disconnected, exit thread
                        break;
                    }
                }
            }
        });

        // store the handle for cleanup in Drop
        *self.thread_handle.lock().unwrap() = Some(handle);

        Ok(())
    }

    /// Utilized by the main program thread to send video frames to
    /// the edge detection thread to be processed
    pub fn submit_frame(&self, frame: &ImageFrame) -> Result<(), Box<dyn Error>> {
        self.frame_sender
            .as_ref()
            .expect("frame_sender should exist until Drop")
            .send(frame.clone())
            .map_err(|_| "EdgeDetector thread disconnected".into())
    }

    /// Using the Sobel operator, processes an image frame for edge detection
    /// after retrieving the grayscale intensity map
    fn process_frame(
        frame: &ImageFrame,
        threshold: f32,
        buffers: &mut ProcessingBuffers,
    ) -> Result<(), Box<dyn Error>> {
        Self::create_intensity_map(frame, &mut buffers.intensity);
        Self::sobel(&buffers.intensity, frame.w, frame.h, &mut buffers.gx, &mut buffers.gy);

        // for each pixel...
        for i in 0..(frame.w * frame.h) {
            // get the strength / intensity of the edge
            buffers.magnitude[i] = (buffers.gx[i] * buffers.gx[i] + buffers.gy[i] * buffers.gy[i]).sqrt();
            // get the direction of the edge
            buffers.angle[i] = buffers.gy[i].atan2(buffers.gx[i]);
        }

        // thin edges & remove edges that are most likely just noise
        Self::non_maximum_suppression(
            &buffers.magnitude,
            &buffers.angle,
            frame.w,
            frame.h,
            threshold,
            &mut buffers.suppression_result,
        );

        // copy suppressed magnitude back to magnitude buffer
        buffers.magnitude.copy_from_slice(&buffers.suppression_result);

        Ok(())
    }

    /// Retrieve the edge information from the `EdgeDetector`
    /// Returns an Arc reference to the edge info (lock-free, no cloning)
    pub fn get_edge_info(&self) -> Arc<EdgeInfo> {
        self.edge_info.load_full()
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Extracts intensity values from an RGB image to be used
    /// for edge detection
    fn create_intensity_map(frame: &ImageFrame, intensity: &mut [f32]) {
        for y in 0..frame.h {
            for x in 0..frame.w {
                if let Some((r, g, b)) = frame.get_pixel(x, y) {
                    let gray = ImageFrame::calculate_intensity((r, g, b));
                    intensity[y * frame.w + x] = gray;
                }
            }
        }
    }

    /// Applies the Sobel operator to a matrix containing the intensities of
    /// a processed `ImageFrame`. This is utilized for edge detection in the
    /// image.
    ///
    /// The Sobel kernels are defined as follows:
    /// - `Gx = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]]`
    /// - `Gy = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]]`
    fn sobel(intensity: &[f32], w: usize, h: usize, gx: &mut [f32], gy: &mut [f32]) {
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let i = y * w + x;

                // skipping over entries w/ 0 due to initialization
                gx[i] = -1.0 * intensity[(y - 1) * w + (x - 1)] + // Gx(0,0)
                        1.0 * intensity[(y - 1) * w + (x + 1)] +  // Gx(0,2)
                        -2.0 * intensity[y * w + (x - 1)] +       // Gx(1,0)
                        2.0 * intensity[y * w + (x + 1)] +        // Gx(1,2)
                        -1.0 * intensity[(y + 1) * w + (x - 1)] + // Gx(2,0)
                        1.0 * intensity[(y + 1) * w + (x + 1)];   // Gx(2,2)

                // ditto
                gy[i] = -1.0 * intensity[(y - 1) * w + (x - 1)] + // Gy(0,0)
                        -2.0 * intensity[(y - 1) * w + x] +       // Gy(0,1)
                        -1.0 * intensity[(y - 1) * w + (x + 1)] + // Gy(0,2)
                        1.0 * intensity[(y + 1) * w + (x - 1)] +  // Gy(2,0)
                        2.0 * intensity[(y + 1) * w + x] +        // Gy(2,1)
                        1.0 * intensity[(y + 1) * w + (x + 1)];   // Gy(2,2)
            }
        }
    }

    /// Performs non-maximum suppression on a gradient magnitude to thin edges.
    ///
    /// By examining each pixel and its neighbors along the gradient direction,
    /// the function determines a local maximum. Only pixels that meet / exceed
    /// the local maximum and exceed the threshold are preserved.
    ///
    /// This will reduce the thickness of edges to a single-pixel width and
    /// remove edge points that are more than likely noise.
    fn non_maximum_suppression(
        magnitude: &[f32],
        angle: &[f32],
        w: usize,
        h: usize,
        threshold: f32,
        result: &mut [f32],
    ) {
        // clear result buffer first
        result.fill(0.0);

        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let i = y * w + x;

                // below magnitude? weak edge, skip
                if magnitude[i] < threshold {
                    continue;
                }

                // normalize to 0-180 degrees
                let angle_deg = (angle[i].to_degrees() + 180.0) % 180.0;

                let (nx1, ny1, nx2, ny2) = if (angle_deg >= 0.0 && angle_deg < 22.5)
                    || (angle_deg >= 157.5 && angle_deg < 180.0)
                {
                    // horizontal edge
                    (x + 1, y, x - 1, y)
                } else if angle_deg >= 22.5 && angle_deg < 67.5 {
                    // forward edge (/)
                    (x + 1, y - 1, x - 1, y + 1)
                } else if angle_deg >= 67.5 && angle_deg < 112.5 {
                    // vertical edge
                    (x, y - 1, x, y + 1)
                } else {
                    // back edge (\)
                    (x - 1, y - 1, x + 1, y + 1)
                };

                // compare with neighboring values
                let n1 = if nx1 < w && ny1 < h {
                    magnitude[ny1 * w + nx1]
                } else {
                    0.0
                };

                let n2 = if nx2 < w && ny2 < h {
                    magnitude[ny2 * w + nx2]
                } else {
                    0.0
                };

                // Keep only local maxima
                if magnitude[i] >= n1 && magnitude[i] >= n2 {
                    result[i] = magnitude[i];
                }
            }
        }
    }
}

impl Drop for EdgeDetector {
    fn drop(&mut self) {
        // signal the thread to stop
        self.stop();

        // drop the sender to close the channel and unblock the receiver
        // this will cause recv() in the thread to return Err and exit
        self.frame_sender.take();

        // now the thread will exit from recv() returning Err
        // wait for the thread to finish
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}
