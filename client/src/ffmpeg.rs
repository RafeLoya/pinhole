use regex::Regex;
use std::error::Error;
use std::process::{Child, Command, Stdio};

/// Determines if `ffmpeg` has been installed and spawns a daemon to feed
/// image frames to the program with default arguments
/// (i.e. no custom arguments passed to `ffmppeg`)
///
/// TODO: greater argument flexibility (alt function)
///
/// # Examples
///
/// ```
/// let mut ffmpeg_proc = match setup_default() {
///     Ok(ffmpeg) => ffmpeg,
///     Err(err) => {
///         eprintln!("failed to initialize ffmpeg: {}", err);
///         return Err(err);
///     }
/// }
/// ```
pub fn setup_default() -> Result<Child, Box<dyn std::error::Error>> {
    match Command::new("ffmpeg").arg("-version").output() {
        Ok(output) => {
            println!(
                "ffmpeg found: {}",
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or_default()
            )
        }
        Err(e) => return Err(format!("ffmpeg not found or not accessible: {}", e).into()),
    }

    let mut cmd = Command::new("ffmpeg");
    os_setup(&mut cmd)?;

    let daemon = match cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(child) => child,
        Err(e) => {
            return Err(format!("failed to spawn ffmpeg process: {}", e).into());
        }
    };
    Ok(daemon)
}

/// Query FFmpeg for DirectShow video devices on Windows.
/// Returns a list like ["USB2.0 HD UVC WebCam", "OBS Virtual Camera"].
fn list_windows_cameras() -> Result<Vec<String>, Box<dyn Error>> {
    // stderr is where FFmpeg prints the table
    let out = Command::new("ffmpeg")
        .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
        .stderr(Stdio::piped())
        .output()?;

    let text = String::from_utf8_lossy(&out.stderr);

    // Example line:
    // [dshow @ 000002600bd5f2c0] "USB2.0 HD UVC WebCam" (video)
    let re = Regex::new(r#"]\s+"([^"]+)"\s+\((video)\)"#)?;

    let cams: Vec<String> = re
        .captures_iter(&text)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_owned()))
        .collect();

    if cams.is_empty() {
        Err("no DirectShow video devices found".into())
    } else {
        Ok(cams)
    }
}

/// Determines the OS of the current system and structures the
/// `ffmpeg` CLI with the appropriate arguments
///
/// TODO: verify Windows / Linux compatibility
fn os_setup(cmd: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(target_os = "macos") {
        println!("MacOS detected");
        cmd.args([
            "-f",
            "avfoundation",
            "-framerate",
            "30",
            "-video_size",
            "640x480",
            "-pixel_format",
            "rgb24",
            "-i",
            "0:none",
            // output opts
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            // latency opts
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-fflags",
            "nobuffer",
            "-flags",
            "low_delay",
            // pipe to stdout
            "pipe:1",
        ]);
    } else if cfg!(target_os = "linux") {
        println!("Linux detected");
        cmd.args([
            "-f",
            "v4l2",
            "-framerate",
            "30",
            "-video_size",
            "640x480",
            "-pixel_format",
            "rgb24",
            "-i",
            "/dev/video0",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-fflags",
            "nobuffer",
            "-flags",
            "low_delay",
            "pipe:1",
        ]);
    } else if cfg!(target_os = "windows") {
        println!("Windows detected");

        // Choose the first physical camera (skip virtual if you like)
        let cams = list_windows_cameras()?;
        let cam_name = cams
            .into_iter()
            .find(|c| !c.contains("Virtual")) // naïve filter
            .unwrap_or_else(|| String::from("default"));

        cmd.args([
            "-f",
            "dshow",
            "-framerate",
            "30",
            "-video_size",
            "640x480",
            "-pixel_format",
            "rgb24",
            "-i",
            &format!("video={}", cam_name),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-fflags",
            "nobuffer",
            "-flags",
            "low_delay",
            "pipe:1",
        ]);
    } else {
        return Err("Current OS not supported".into());
    }

    Ok(())
}
