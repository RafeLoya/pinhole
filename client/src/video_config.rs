pub struct VideoConfig {
    pub camera_width: usize,
    pub camera_height: usize,
    pub ascii_width: usize,
    pub ascii_height: usize,
    pub edge_threshold: f32,
    pub contrast: f32,
    pub brightness: f32,
}

impl VideoConfig {
    pub fn default() -> Self {
        Self {
            camera_width: 640,
            camera_height: 480,
            ascii_width: 120,
            ascii_height: 40,
            edge_threshold: 20.0,  // Use a single consistent default
            contrast: 1.5,
            brightness: 0.0,
        }
    }

    pub fn new(
        camera_width: usize,
        camera_height: usize,
        ascii_width: usize,
        ascii_height: usize,
        edge_threshold: f32,
        contrast: f32,
        brightness: f32,
    ) -> Self {

        Self {
            camera_width,
            camera_height,
            ascii_width,
            ascii_height,
            edge_threshold,
            contrast,
            brightness,
        }
    }
}