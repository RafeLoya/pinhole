extern crate alloc;

mod text_converter;
mod text_renderer;
mod camera;
mod client;
mod terminal;
mod config;
mod edge_detector;
mod ffmpeg;
mod image_frame;
mod mock_frame_generator;
mod room_client;
mod cli;

use crate::cli::{apply_dimension_overrides, apply_image_processing_overrides, apply_source_override, Args, Command};
use crate::client::Client;
use crate::config::PinholeConfig;
use crate::mock_frame_generator::PatternType;
use crate::room_client::RoomClient;
use crate::terminal::TerminalGuard;
use crate::terminal::TerminalInfo;
use clap::Parser;
use crossterm::event::{read, Event, KeyCode};
use ffmpeg_sidecar::command::ffmpeg_is_installed;
use ffmpeg_sidecar::download::auto_download;
use ffmpeg_sidecar::version::ffmpeg_version;
use std::error::Error;
use tokio::signal;
use tokio_util::sync::CancellationToken;


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // check for FFmpeg availability & optionally install
    ffmpeg_check()?;

    // === SHUTDOWN HANDLER =======================================================================
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            cancel_token_clone.cancel();
        }
    });

    // === CONFIGURATION ==========================================================================
    let mut config = if args.config.exists() {
        println!("Loading config from: {}", args.config.display());
        PinholeConfig::from_file(&args.config)?
    } else {
        println!(
            "Config file not found at {}, using defaults",
            args.config.display()
        );
        PinholeConfig::default()
    };

    // detect terminal capabilities
    let _term_info = TerminalInfo::detect(config.terminal.clone())?;

    // various config & CLI overrides
    apply_dimension_overrides(&mut config, &args);
    apply_source_override(&mut config, &args);
    apply_image_processing_overrides(&mut config, &args);

    let pattern_type = args.test_pattern.map(PatternType::from);
    if pattern_type.is_some() {
        println!("Using test pattern: {:?}", args.test_pattern);
    }

    // === COMMAND DISPATCH =======================================================================
    match args.command {
        Command::Host {
            api_url,
            tcp_addr,
            udp_addr,
        } => {
            run_host(config, pattern_type, cancel_token, &api_url, &tcp_addr, &udp_addr).await?;
        }

        Command::Join {
            room_code,
            api_url,
            tcp_addr,
            udp_addr,
        } => {
            run_join(
                config,
                pattern_type,
                cancel_token,
                &room_code,
                &api_url,
                &tcp_addr,
                &udp_addr,
            )
            .await?;
        }

        Command::Solo => {
            run_solo(config, pattern_type, cancel_token).await?;
        }

        // legacy session join, join command preferred
        Command::Connect {
            tcp_addr,
            udp_addr,
            session_id,
        } => {
            run_connect(config, pattern_type, cancel_token, tcp_addr, udp_addr, session_id).await?;
        }
    }

    println!("pinhole gracefully shut down");
    Ok(())
}


/// Check if FFmpeg is installed. If not, ask if user would want to download
/// through pinhole. If so, FFmpeg will be installed through `ffmpeg-sidecar`.
/// Else, download instructions are given and program exits with error
fn ffmpeg_check() -> Result<(), Box<dyn Error>> {
    if ffmpeg_is_installed() {
        if let Ok(version) = ffmpeg_version() {
            println!("FFmpeg version: {}", version);
        }
        return Ok(())
    }

    println!("[ERROR] FFmpeg is not installed or could not be found.");
    println!("        Would you like to download the CLI through pinhole? (y/n)");

    loop {
        if let Event::Key(event) = read()? {
            match event.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    println!("[INFO] attempting to download FFmpeg...");
                    auto_download()?;
                    if let Ok(version) = ffmpeg_version() {
                        println!("[INFO] FFmpeg downloaded successfully, version: {}", version);
                    }
                    return Ok(())
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    println!("[INFO] declined to download FFmpeg.");
                    println!("       consult https://www.ffmpeg.org/ for download instructions, or:");
                    #[cfg(target_os = "macos")]
                    println!("       install with: brew install ffmpeg");
                    #[cfg(target_os = "linux")]
                    println!("       install with your distro's package manager (ex. sudo apt install ffmpeg)");
                    #[cfg(target_os = "windows")]
                    println!("       install with: winget install ffmpeg");

                    return Err("FFmpeg is required but not installed".into());
                }
                _ => {}
            }
        }
    }
}

/// Host a session: create room code and wait for peer.
async fn run_host(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    api_url: &str,
    tcp_addr: &str,
    udp_addr: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Registering room with server...");

    let room_client = RoomClient::new(api_url);

    // check server health first
    if !room_client.health_check().await {
        return Err(format!("Cannot reach room API at {}", api_url).into());
    }

    // create room
    let room = room_client.create_room().await?;
    println!();
    println!("Room Code: {:^25}", room.room_code);
    println!();
    println!("Share this code with your peer to connect.");
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Waiting for peer...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        tcp_addr.to_string(),
        udp_addr.to_string(),
        room.session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}

/// Join a session using a room code.
async fn run_join(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    room_code: &str,
    api_url: &str,
    tcp_addr: &str,
    udp_addr: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Looking up room code: {}", room_code);

    let room_client = RoomClient::new(api_url);

    // lookup room
    let room = room_client.lookup_room(room_code).await?;
    println!("Found session: {}", room.session_id);
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Connecting...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        tcp_addr.to_string(),
        udp_addr.to_string(),
        room.session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}

async fn cleanup(cancel_token: CancellationToken, _terminal_guard: TerminalGuard, client: Client) {
    tokio::select! {
        result = client.run() => {
            if let Err(e) = result {
                drop(_terminal_guard);
                eprintln!("Error: {}", e);
            }
        }
        _ = cancel_token.cancelled() => {}
    }
}

/// Run in solo mode (local preview).
async fn run_solo(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
) -> Result<(), Box<dyn Error>> {
    println!("Running in solo mode (local preview only)");
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Starting in 1 second...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        String::new(),
        String::new(),
        String::new(),
        pattern_type,
        config,
        cancel_token.clone(),
    );

    tokio::select! {
        result = client.run_solo() => {
            if let Err(e) = result {
                drop(_terminal_guard);
                eprintln!("Error in solo mode: {}", e);
            }
        }
        _ = cancel_token.cancelled() => {}
    }

    Ok(())
}

/// Direct connection with manual session ID (legacy mode).
async fn run_connect(
    mut config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    tcp_addr: Option<String>,
    udp_addr: Option<String>,
    session_id: Option<String>,
) -> Result<(), Box<dyn Error>> {
    // apply overrides from CLI
    if let Some(addr) = tcp_addr {
        config.network.tcp_addr = addr;
    }
    if let Some(addr) = udp_addr {
        config.network.udp_addr = addr;
    }
    if let Some(id) = session_id {
        config.network.session_id = id;
    }

    // generate random session ID if not provided
    let session_id = if config.network.session_id.is_empty() {
        let rand_id: u32 = rand::random();
        format!("session-{}", rand_id)
    } else {
        config.network.session_id.clone()
    };

    println!("Connecting to session: {}", session_id);
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Starting in 1 second...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        config.network.tcp_addr.clone(),
        config.network.udp_addr.clone(),
        session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error in network mode: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}