mod ffmpeg;
mod camera;
mod ascii_renderer;
mod ascii_frame;
mod image_frame;
mod ascii_converter;
mod edge_detector;
mod video_config;

use crate::ascii_converter::AsciiConverter;
use crate::ascii_frame::AsciiFrame;
use crate::ascii_renderer::AsciiRenderer;
use crate::camera::Camera;
use crate::image_frame::ImageFrame;

use crate::video_config::VideoConfig;
use std::time::Duration;
use std::thread;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph, Gauge, Widget},
    layout::{Alignment, Layout, Constraint, Direction},
    Terminal, Frame,
    style::{Style, Color},
};
use std::io::{self, stdout};
use std::time::Instant;

fn show_startup_screen() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let total_frames = 30;
    let mut frame_count = 0;

    while frame_count <= total_frames {
        terminal.draw(|f| {
            draw_loading(f, frame_count, total_frames);
        })?;
        frame_count += 1;
        thread::sleep(Duration::from_millis(100));
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_loading(f: &mut Frame, frame_count: usize, total_frames: usize) {
    let size = f.size();

    // Divide terminal vertically: title, loading text, progress bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(5)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(3), // loading text
            Constraint::Length(3), // progress bar
        ])
        .split(size);

    // Block title
    let block = Block::default()
        .title("ASCII Cam")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL);
    f.render_widget(block, size);

    // Animated dots in loading message
    let dots = ".".repeat((frame_count % 4) as usize);
    let loading_text = format!("Initializing camera{}", dots);
    let paragraph = Paragraph::new(loading_text)
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center);
    f.render_widget(paragraph, chunks[1]);

    // Progress bar logic
    let progress_ratio = frame_count as f64 / total_frames as f64;
    let percent = (progress_ratio * 100.0).min(100.0) as u16;
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Loading"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent(percent);
    f.render_widget(gauge, chunks[2]);
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    show_startup_screen()?;
    let config = VideoConfig::new(
        640,
        480,
        120,
        40,
        127.50,
        1.5,
        0.0
    );

    let mut camera = Camera::new(config.camera_width, config.camera_height)?;

    let mut image_frame = ImageFrame::new(config.camera_width, config.camera_height, 3)?;
    let mut ascii_frame = AsciiFrame::new(config.ascii_width, config.ascii_height, ' ')?;

    let converter = AsciiConverter::new(
        AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
        config.camera_width,
        config.camera_height,
        config.edge_threshold,
        config.contrast,
        config.brightness
    )?;

    let mut renderer = AsciiRenderer::new()?;

    loop {
        if let Err(e) = camera.capture_frame(&mut image_frame) {
            eprintln!("failed while capturing frame: {}", e);
            break;
        }

        if let Err(e) = converter.convert(&image_frame, &mut ascii_frame) {
            eprintln!("failed while converting frame: {}", e);
            break;
        }

        if let Err(e) = renderer.render(&ascii_frame) {
            eprintln!("failed while rendering frame: {}", e);
            break;
        }

        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}