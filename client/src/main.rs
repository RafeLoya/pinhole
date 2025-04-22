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
    let start_time = Instant::now();

    // ASCII art for the title
    let ascii_title = r#"
    _    ____   ____ ___ ___    ____    _    __  __
   / \  / ___| / ___|_ _|_ _|  / ___|  / \  |  \/  |
  / _ \ \___ \| |    | | | |  | |     / _ \ | |\/| |
 / ___ \ ___) | |___ | | | |  | |___ / ___ \| |  | |
/_/   \_\____/ \____|___|___|  \____/_/   \_\_|  |_|
    "#;

    while frame_count <= total_frames {
        terminal.draw(|f| {
            draw_enhanced_loading(f, frame_count, total_frames, ascii_title, start_time);
        })?;
        frame_count += 1;
        thread::sleep(Duration::from_millis(80)); // Faster animation
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
fn draw_enhanced_loading(
    f: &mut Frame,
    frame_count: usize,
    total_frames: usize,
    ascii_title: &str,
    start_time: Instant
) {
    let size = f.size();

    // Divide terminal vertically
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(7),  // ASCII art title
            Constraint::Length(1),  // Spacing
            Constraint::Length(1),  // Status text
            Constraint::Length(3),  // Progress bar
            Constraint::Length(1),  // Spacing
            Constraint::Length(3),  // Animation frame
            Constraint::Min(0),     // Rest of the space
        ])
        .split(size);

    // Main block with border
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" ASCII Cam ")
        .title_alignment(Alignment::Center)
        .title_style(Style::default().fg(Color::Yellow));
    f.render_widget(main_block, size);

    // ASCII art title
    let title = Paragraph::new(ascii_title)
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Status messages cycling
    let status_messages = vec![
        "Initializing camera...",
        "Configuring edge detection...",
        "Setting up ASCII conversion...",
        "Preparing render pipeline..."
    ];
    let current_status = status_messages[frame_count % status_messages.len()];
    let status = Paragraph::new(current_status)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    f.render_widget(status, chunks[2]);

    // Progress bar with rainbow colors
    let progress_ratio = frame_count as f64 / total_frames as f64;
    let percent = (progress_ratio * 100.0).min(100.0) as u16;

    // Rainbow color effect
    let colors = [Color::Red, Color::Yellow, Color::Green, Color::Cyan, Color::Blue, Color::Magenta];
    let color_index = (frame_count / 3) % colors.len();
    let current_color = colors[color_index];

    let gauge = Gauge::default()
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Loading ")
            .border_style(Style::default().fg(Color::Gray)))
        .gauge_style(Style::default().fg(current_color).bg(Color::Black))
        .ratio(progress_ratio)
        .label(format!("{:>3}%", percent));
    f.render_widget(gauge, chunks[3]);

    // Animated ASCII spinner
    let spinner_chars = vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_char = spinner_chars[frame_count % spinner_chars.len()];

    let elapsed = start_time.elapsed();
    let spinner_text = format!(
        "{} Starting in {:02}.{:02}s",
        spinner_char,
        elapsed.as_secs(),
        elapsed.subsec_millis() / 10
    );

    let spinner = Paragraph::new(spinner_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(spinner, chunks[5]);

    // Bottom tips section
    let tips = vec![
        "Tip: Adjust brightness with +/- keys",
        "Tip: Press 'q' to quit anytime",
        "Tip: Use arrow keys to adjust contrast",
    ];
    let current_tip = tips[(frame_count / 10) % tips.len()];
    let tip_text = Paragraph::new(current_tip)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    // Create a bottom chunk for tips
    let bottom_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(chunks[6]);

    f.render_widget(tip_text, bottom_chunks[1]);
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
    let dots = ".".repeat((frame_count /10) %4 as usize);
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