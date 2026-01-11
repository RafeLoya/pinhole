use crate::config::PinholeConfig;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use std::error::Error;

/// Setup FFmpeg based on the provided configuration
pub fn setup_from_config(config: &PinholeConfig) -> Result<FfmpegChild, Box<dyn Error>> {
    let source = &config.video.source;

    match source.r#type.as_str() {
        "webcam" => setup_webcam(config),
        "screen" => setup_screen(config),
        "file" => setup_file(config),
        "custom" => setup_custom(config),
        _ => Err(format!("unknown source type: {}", source.r#type).into()),
    }
}

/// Setup FFmpeg for webcam capture
fn setup_webcam(config: &PinholeConfig) -> Result<FfmpegChild, Box<dyn Error>> {
    let webcam = &config.video.source.webcam;
    let ffmpeg_cfg = &config.video.ffmpeg;

    let mut cmd = FfmpegCommand::new();

    if cfg!(target_os = "macos") {
        println!("MacOS detected - using avfoundation");
        cmd.format("avfoundation")
            .args(["-framerate", &webcam.framerate.to_string()])
            .args(["-video_size", &format!("{}x{}", webcam.width, webcam.height)])
            .args(["-pixel_format", &webcam.pixel_format])
            .input(&webcam.device);
    } else if cfg!(target_os = "linux") {
        println!("Linux detected - using v4l2");
        cmd.format("v4l2")
            .args(["-framerate", &webcam.framerate.to_string()])
            .args(["-video_size", &format!("{}x{}", webcam.width, webcam.height)])
            .args(["-pixel_format", &webcam.pixel_format])
            .input(&webcam.device);
    } else if cfg!(target_os = "windows") {
        println!("Windows detected - using dshow");
        cmd.format("dshow")
            .args(["-framerate", &webcam.framerate.to_string()])
            .args(["-video_size", &format!("{}x{}", webcam.width, webcam.height)])
            .args(["-vcodec", "mjpeg"])
            .input(&webcam.device);
    } else {
        return Err("Current OS not supported".into());
    }

    // output format and low-latency options
    cmd.format("rawvideo")
        .pix_fmt(&webcam.pixel_format)
        .args(["-probesize", &ffmpeg_cfg.probesize.to_string()])
        .args(["-analyzeduration", &ffmpeg_cfg.analyzeduration.to_string()])
        .args(["-fflags", &ffmpeg_cfg.fflags])
        .args(["-flags", &ffmpeg_cfg.flags])
        .output("pipe:1");

    let child = cmd.spawn()?;
    Ok(child)
}

/// Setup FFmpeg for screen capture
fn setup_screen(config: &PinholeConfig) -> Result<FfmpegChild, Box<dyn Error>> {
    let screen = &config.video.source.screen;
    let ffmpeg_cfg = &config.video.ffmpeg;

    let mut cmd = FfmpegCommand::new();

    if cfg!(target_os = "macos") {
        println!("MacOS screen capture - using avfoundation");
        cmd.format("avfoundation")
            .args(["-framerate", &screen.framerate.to_string()])
            .input(&screen.device);
    } else if cfg!(target_os = "linux") {
        println!("Linux screen capture - using x11grab");
        cmd.format("x11grab")
            .args(["-framerate", &screen.framerate.to_string()])
            .args(["-video_size", &format!("{}x{}", screen.width, screen.height)])
            .input(&screen.device);
    } else if cfg!(target_os = "windows") {
        println!("Windows screen capture - using gdigrab");
        cmd.format("gdigrab")
            .args(["-framerate", &screen.framerate.to_string()])
            .input(&screen.device);
    } else {
        return Err("Current OS not supported for screen capture".into());
    }

    // output format and low-latency options
    cmd.format("rawvideo")
        .pix_fmt("rgb24")
        .args(["-probesize", &ffmpeg_cfg.probesize.to_string()])
        .args(["-analyzeduration", &ffmpeg_cfg.analyzeduration.to_string()])
        .args(["-fflags", &ffmpeg_cfg.fflags])
        .args(["-flags", &ffmpeg_cfg.flags])
        .output("pipe:1");

    let child = cmd.spawn()?;
    Ok(child)
}

/// Setup FFmpeg for file playback
fn setup_file(config: &PinholeConfig) -> Result<FfmpegChild, Box<dyn Error>> {
    let file = &config.video.source.file;
    let ffmpeg_cfg = &config.video.ffmpeg;

    if file.path.is_empty() {
        return Err("file path is empty in configuration".into());
    }

    println!("Playing file: {}", file.path);

    let mut cmd = FfmpegCommand::new();

    cmd.input(&file.path)
        .format("rawvideo")
        .pix_fmt("rgb24")
        .args(["-fflags", &ffmpeg_cfg.fflags])
        .args(["-flags", &ffmpeg_cfg.flags])
        .output("pipe:1");

    let child = cmd.spawn()?;
    Ok(child)
}

/// Setup FFmpeg with custom arguments
fn setup_custom(config: &PinholeConfig) -> Result<FfmpegChild, Box<dyn Error>> {
    let custom = &config.video.source.custom;

    if custom.args.is_empty() {
        return Err("custom args are empty in configuration".into());
    }

    println!("Using custom FFmpeg arguments");

    let mut cmd = FfmpegCommand::new();
    cmd.args(&custom.args);

    let child = cmd.spawn()?;
    Ok(child)
}

/// Deprecated: Legacy setup function for backwards compatibility
/// Use `setup_from_config` instead
#[deprecated(note = "Use setup_from_config instead")]
pub fn setup_default() -> Result<FfmpegChild, Box<dyn Error>> {
    let config = PinholeConfig::default();
    setup_from_config(&config)
}
