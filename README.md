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

**pinhole** is a video chat application that functions completely within a terminal emulator.

The video feed from two peers in the same session is forwarded between one another in a UTF character representation. With just a network, a terminal emulator, and a way to record I-frames, you can send, receive, and render live video!

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
# preview your webcam
cargo run --bin pinhole -- solo

# preview with test pattern
cargo run --bin pinhole -- -p checkerboard solo

# preview with custom config
cargo run --bin pinhole -- -c my-config.toml solo
```

This is perfect for:
- Testing your camera setup
- Verifying your config file settings
- Previewing different video sources
- Debugging your rendering settings

## Network Mode (Video Chat)

Start a server, then connect two peers to the same session.

```shell
# start a server (leave this running)
cargo run --release --bin pinhole-server
```

Join with a room code (recommended):

```shell
# peer A: create a room and get a code
cargo run --release --bin pinhole -- host

# peer B: join with the printed code
cargo run --release --bin pinhole -- join swift-river-42
```

Or connect directly with a manual session id:

```shell
cargo run --release --bin pinhole -- connect -t <SERVER_TCP> -u <SERVER_UDP> -s <SESSION_ID>

# example with local server
cargo run --release --bin pinhole -- connect -t 127.0.0.1:8080 -u 127.0.0.1:4433 -s my-session
```

## Dimension Control **(WIP)**

Control the ASCII frame dimensions using presets or custom values:

```shell
# use a preset size
cargo run --bin pinhole -- --preset small solo    # 80x24 (UDP safe)
cargo run --bin pinhole -- --preset medium solo   # 120x40 (UDP safe, default)
cargo run --bin pinhole -- --preset large solo    # 160x50 (may have packet loss)
cargo run --bin pinhole -- --preset xlarge solo   # 200x60 (currently not working w/ UDP)

# or specify exact dimensions
cargo run --bin pinhole -- --width 100 --height 30 solo

# combine preset with overrides (width/height override preset)
cargo run --bin pinhole -- --preset large --height 60 solo
```

**Current UDP Limitations:**
- **Small/Medium presets** (~1400-5000 bytes): Work reliably over UDP
- **Large preset** (~8000 bytes): May experience packet fragmentation and loss
- **XLarge preset** (~12000 bytes): Will not work over UDP due to packet fragmentation

**Note:** The diff compression significantly reduces bandwidth after the first frame, but initial Full frames must still fit within UDP limits.

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
