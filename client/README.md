If wanting to test locally with your webcam, enter the following:

 ```bash
 # Solo mode (local preview, no server connection)
 cargo run --release --bin pinhole -- --solo
 ```

 To connect to a session with a live server, enter the following:

 ```bash
 # Network mode
 cargo run --release --bin pinhole -- -t <TCP_PORT> -u <UDP_PORT> -s <SESSION_ID>
 ```

 where:
 - `TCP_PORT` and `UDP_PORT` is port of your choosing on 127.0.0.1
 - `SESSION_ID` can be any string (for now)