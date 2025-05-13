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

Pinhole is a video chat application that functions completely within a shell.

The video feed from two peers in the same session is forwarded between one another in a custom UTF-8 character representation. With just a network, a shell, and a way to record I-frames, you can send, receive, and render live video!

This repository contains a server and client binary, where a server facilitates the actual connection between two clients and the forwarding of their video data. End users will likely want to use the client executable, provided a server is up and running.

# Requirements

The client binary directly uses the FFmpeg CLI, which can be downloaded from the [official website](https://ffmpeg.org/download.html). The website itself only provides the source code, but if you are not interested in building from source, links are provided to find it as an executable.

# Installation

## Building From Source

After cloning the repository, build with `cargo` with a release flag and use the executable(s) as you see fit:

```shell
cargo build --release

# or, if you are only interested in one executable:
cargo build --release --bin client
cargo build --release --bin server
```
