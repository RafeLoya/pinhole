pub mod text_frame;
pub mod frame_encoding;
pub mod frame_pixel;
pub mod logger;

/// Maximum safe UDP packet size to avoid IP fragmentation
/// Ethernet MTU (1500) - IP header (20) - UDP header (8) = 1472 bytes
/// Using conservative 1400 to account for network overhead and tunneling
pub const MAX_UDP_PACKET_SIZE: usize = 1400;