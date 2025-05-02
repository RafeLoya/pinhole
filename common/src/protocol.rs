use std::error::Error;
use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use std::time::Instant;
use bcrypt;

use crate::ascii_frame::AsciiFrame;

pub type UserId = String;
pub type UserInfo = String;

#[derive(Serialize, Deserialize, Clone)]
pub struct VideoFrame {
    /// Width of frame
    pub w: usize,
    /// Height of frame
    pub h: usize,
    /// For latency calculation
    pub timestamp: u64,
    /// Data (characters representing 'pixels' of frame)
    pub data: Vec<char>,
}

impl VideoFrame {
    pub fn from_ascii_frame(frame: &AsciiFrame) -> VideoFrame {
        let now = Instant::now();
        Self {
            w: frame.w,
            h: frame.h,
            data: frame.chars().to_vec(),
            timestamp: now.elapsed().as_micros() as u64,
        }
    }
    
    pub fn to_ascii_frame(&self) -> Result<AsciiFrame, Box<dyn Error>> {
        let mut frame = AsciiFrame::new(self.w, self.h, ' ')?;
        
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                if i < self.data.len() {
                    frame.set_char(x, y, self.data[i]);
                }
            }
        }
        
        Ok(frame)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MessageType {
    
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub username: String,
    pub address: SocketAddr,
    pub status: UserStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq)]
pub enum UserStatus {
    Online,
    InCall,
    Offline
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub msg_type: MessageType,
}