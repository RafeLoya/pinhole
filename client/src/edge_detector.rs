use crate::image_frame::ImageFrame;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;

// TODO: Look into Robert's Cross operator as potential alternative (if slow performance)
// TODO: Remove `.unwrap()`s in the future for error recovery
// TODO: Allow user to influence `threshold` data member

pub struct EdgeInfo {
    /// the strength / intensity of an edge, if it exists
    pub magnitude: Vec<f32>,
    /// the angle of an edge, if it exists
    pub angle: Vec<f32>,
    /// the width of the camera it will receive image frames from
    pub w: usize,
    /// the height of the camera it will receive image frames from
    pub h: usize,
}

pub struct EdgeDetector {
    /// The edge magnitudes and angles of the latest processed `ImageFrame`.
    edge_info: Arc<Mutex<EdgeInfo>>,
    /// The raw image data of the latest processed `ImageFrame`
    frame_buffer: Arc<Mutex<Vec<u8>>>,
    /// Flag, indicates to `EdgeDetector` that there is a new `ImageFrame`
    /// loaded in `frame_buffer`
    new_frame_available: Arc<Mutex<bool>>,
    /// Minimum gradient magnitude threshold.
    /// Operates from 0.0 to 255.0
    threshold: f32,
    /// Control flag, will terminate the edge detection thread when `false`
    running: Arc<Mutex<bool>>,
}

impl EdgeDetector {
    /// default `threshold` value if none is provided
    pub const DEFAULT_EDGE_THRESHOLD: f32 = 20.0;

    pub fn new(w: usize, h: usize, threshold: f32) -> Self {
        let edge_info = Arc::new(Mutex::new(EdgeInfo {
            magnitude: vec![0.0; w * h],
            angle: vec![0.0; w * h],
            w,
            h,
        }));

        let frame_buffer = Arc::new(Mutex::new(Vec::<u8>::with_capacity(w * h * 3)));
        let new_frame_available = Arc::new(Mutex::new(false));
        let running = Arc::new(Mutex::new(true));

        Self {
            edge_info,
            frame_buffer,
            new_frame_available,
            threshold,
            running,
        }
    }

    /// Launches the edge detection processing thread.
    ///
    /// This processing thread continuously checks for new frames produced by
    /// a camera, then processes them with various algorithms to obtain
    /// edge information. Communication between threads is enforced by
    /// shared state protected with mutexes.
    ///
    /// # Returns
    ///
    /// `JoinHandle` for the edge detection processing thread, to manage
    /// or complete its lifetime.
    pub fn start(
        &self,
        cam_w: usize,
        cam_h: usize,
    ) -> Result<thread::JoinHandle<()>, Box<dyn Error>> {
        let edge_info = Arc::clone(&self.edge_info);
        let frame_buffer = Arc::clone(&self.frame_buffer);
        let new_frame_flag = Arc::clone(&self.new_frame_available);
        let running = Arc::clone(&self.running);
        let threshold = self.threshold;

        let handle = thread::spawn(move || {
            while *running.lock().unwrap() {
                let process_frame = {
                    let mut flag = new_frame_flag.lock().unwrap();
                    let should_proc = *flag;
                    *flag = false;
                    should_proc
                };

                if process_frame {
                    let frame_data = frame_buffer.lock().unwrap().clone();

                    let temp_frame = ImageFrame {
                        w: cam_w,
                        h: cam_h,
                        bytes_per_pixel: 3,
                        buffer: frame_data,
                    };

                    if let Ok((magnitude, angle)) = Self::process_frame(&temp_frame, threshold) {
                        let mut info = edge_info.lock().unwrap();
                        info.magnitude = magnitude;
                        info.angle = angle;
                    }
                } else {
                    //thread::sleep(Duration::from_millis(5));
                }
            }
        });

        Ok(handle)
    }

    /// Utilized by the main program thread to send video frames to
    /// the edge detection thread to be processed
    pub fn submit_frame(&self, frame: &ImageFrame) -> Result<(), Box<dyn Error>> {
        let mut buffer = self.frame_buffer.lock().unwrap();

        buffer.clear();
        buffer.extend_from_slice(frame.buffer());

        let mut flag = self.new_frame_available.lock().unwrap();
        *flag = true;

        Ok(())
    }

    /// Using the Sobel operator, processes an image frame fo edge detection
    /// after retrieving the grayscale intensity map
    fn process_frame(
        frame: &ImageFrame,
        threshold: f32,
    ) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
        let intensity = Self::create_intensity_map(frame);
        let (gx, gy) = Self::sobel(&intensity, frame.w, frame.h);

        let mut magnitude = vec![0.0; frame.w * frame.h];
        let mut angle = vec![0.0; frame.w * frame.h];

        // for each pixel...
        for i in 0..gx.len() {
            // get the strength / intensity of the edge
            magnitude[i] = (gx[i] * gx[i] + gy[i] * gy[i]).sqrt();
            // get the direction of the edge
            angle[i] = gy[i].atan2(gx[i]);
        }

        // thin edges & remove edges that are most likely just noise
        let magnitude =
            Self::non_maximum_suppression(&magnitude, &angle, frame.w, frame.h, threshold);

        Ok((magnitude, angle))
    }

    pub fn get_edge_info(&self) -> Result<EdgeInfo, Box<dyn Error>> {
        let edge_info = self.edge_info.lock().unwrap();

        Ok(EdgeInfo {
            magnitude: edge_info.magnitude.clone(),
            angle: edge_info.angle.clone(),
            w: edge_info.w,
            h: edge_info.h,
        })
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Extracts intensity values from an RGB image to be used
    /// for edge detection
    fn create_intensity_map(frame: &ImageFrame) -> Vec<f32> {
        let mut intensity = vec![0.0; frame.w * frame.h];

        for y in 0..frame.h {
            for x in 0..frame.w {
                if let Some((r, g, b)) = frame.get_pixel(x, y) {
                    let gray = ImageFrame::calculate_intensity((r, g, b));
                    intensity[y * frame.w + x] = gray;
                }
            }
        }

        intensity
    }

    /// Applies the Sobel operator to a matrix containing the intensities of
    /// a processed `ImageFrame`. This is utilized for edge detection in the
    /// image.
    ///
    /// The Sobel kernels are defined as follows:
    /// - `Gx = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]]`
    /// - `Gy = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]]`
    fn sobel(intensity: &[f32], w: usize, h: usize) -> (Vec<f32>, Vec<f32>) {
        let mut gx = vec![0.0; w * h];
        let mut gy = vec![0.0; w * h];

        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let i = y * w + x;

                // skipping over entries w/ 0 due to initialization
                gx[i] = -1.0 * intensity[(y - 1) * w + (x - 1)] + // Gx(0,0)
                        1.0 * intensity[(y - 1) * w + (x + 1)] +  // Gx(0,2)
                        -2.0 * intensity[y * w + (x - 1)] +       // Gx(1,0)
                        2.0 * intensity[y * w + (x + 1)] +        // Gx(1,2)
                        -1.0 * intensity[(y + 1) * w + (x - 1)] + // Gx(2,0)
                        1.0 * intensity[(y + 1) * w + (x + 1)]; // Gx(2,2)

                gy[i] = -1.0 * intensity[(y - 1) * w + (x - 1)] + // Gy(0,0)
                        -2.0 * intensity[(y - 1) * w + x] +       // Gy(0,1)
                        -1.0 * intensity[(y - 1) * w + (x + 1)] + // Gy(0,2)
                        1.0 * intensity[(y + 1) * w + (x - 1)] +  // Gy(2,0)
                        2.0 * intensity[(y + 1) * w + x] +        // Gy(2,1)
                        1.0 * intensity[(y + 1) * w + (x + 1)]; // Gy(2,2)
            }
        }

        (gx, gy)
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
    ) -> Vec<f32> {
        let mut result = vec![0.0; w * h];

        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let i = y * w + x;

                // below magnitude? weak edge, skip
                if magnitude[i] < threshold {
                    continue;
                }

                // normalize to 0-180 degrees
                let angle_deg = (angle[i].to_degrees() + 180.0) % 180.0;

                let (nx1, ny1, nx2, ny2) = if (0.0..22.5).contains(&angle_deg)
                    || (157.5..180.0).contains(&angle_deg)
                {
                    // horizontal edge
                    (x + 1, y, x - 1, y)
                } else if (22.5..67.5).contains(&angle_deg) {
                    // forward edge (/)
                    (x + 1, y - 1, x - 1, y + 1)
                } else if (67.5..112.5).contains(&angle_deg) {
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

        result
    }
}
