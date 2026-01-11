```
██████╗ ██╗███╗   ██╗██╗  ██╗ ██████╗ ██╗     ███████╗
██╔══██╗██║████╗  ██║██║  ██║██╔═══██╗██║     ██╔════╝
██████╔╝██║██╔██╗ ██║███████║██║   ██║██║     █████╗  
██╔═══╝ ██║██║╚██╗██║██╔══██║██║   ██║██║     ██╔══╝  
██║     ██║██║ ╚████║██║  ██║╚██████╔╝███████╗███████╗
╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝
```

---

# About

**pinhole** is a video chat application that functions completely within a shell.

The video feed from two peers in the same session is forwarded between one another in a UFT character representation. With just a network, a shell, and a way to record I-frames, you can send, receive, and render live video!

This repository contains a server and client binary, where a server facilitates the actual connection between two clients and the forwarding of their video data. End users will likely want to use the client executable, provided a server is up and running.

# Requirements

FFmpeg is automatically downloaded and installed by the pinhole client if it is not already installed.

Alternatively, it can be installed from the [official website](https://ffmpeg.org/download.html).

# Installation

pinhole supports macOS, Linux, and Windows.

## Building From Source

After cloning the repository, build with `cargo` with a release flag and use the executable(s) as you see fit:

```shell
cargo build --release

# or, if you are only interested in one executable:
cargo build --release --bin pinhole
cargo build --release --bin pinhole-server
```

# Usage

## Solo Mode (Local Preview)

Test your webcam, screen capture, or video file without connecting to a server:

```shell
# Preview your webcam
cargo run --bin pinhole -- --solo

# Preview with test pattern
cargo run --bin pinhole -- --solo -p checkerboard

# Preview with custom config
cargo run --bin pinhole -- --solo -c my-config.toml
```

This is perfect for:
- Testing your camera setup
- Verifying your config file settings
- Previewing different video sources
- Debugging your rendering settings

## Network Mode (Video Chat)

Connect to a server and join a session with another peer:

```shell
# Join a session (requires running server)
cargo run --release --bin pinhole -- -t <SERVER_TCP> -u <SERVER_UDP> -s <SESSION_ID>

# Example with local server
cargo run --release --bin pinhole -- -t 127.0.0.1:8080 -u 127.0.0.1:4433 -s my-session
```

## Configuration

Create a `pinhole.toml` file to configure video sources, ASCII rendering, and more:

```toml
[video.source]
type = "webcam"  # or "screen", "file", "custom"

[video.source.webcam]
# macOS: "0:none"
# Linux: "/dev/video0"
# Windows: "video=Integrated Camera" (run ffmpeg -list_devices true -f dshow -i dummy to find yours)
device = "0:none"
width = 640
height = 480
framerate = 30

[ascii]
width = 120
height = 40

[image_processing]
edge_threshold = 127.5
contrast = 1.5
brightness = 0.0
```

See `pinhole.toml` for a complete example with all available options.

### Finding Your Webcam Device (Windows)

On Windows, you need to specify your camera's exact name:

```shell
ffmpeg -list_devices true -f dshow -i dummy 
```

Then update your config:
```toml
[video.source.webcam]
device = "video=Your Camera Name Here"
```
