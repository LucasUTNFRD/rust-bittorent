use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use crate::SOCKET_PATH; //TODO: Implement our own serialization

#[derive(Debug, Encode, Decode)] // Add Encode, Decode
pub enum DaemonMsg {
    AddTorrent(String),
}

#[derive(Debug, Encode, Decode)] // Add Encode, Decode
pub enum DaemonResponse {
    Success(String),
}

pub fn send_daemon_message(msg: DaemonMsg) -> Result<DaemonResponse> {
    // Connect to the Unix socket
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .with_context(|| format!("Failed to connect to daemon at {}", SOCKET_PATH))?;

    let config = config::standard(); // Get standard configuration
    let serialized = bincode::encode_to_vec(&msg, config) // Use encode_to_vec
        .context("Failed to serialize daemon message")?;

    // Send message length as u32 first (to create a framing protocol)
    let msg_len = serialized.len() as u32;
    stream
        .write_all(&msg_len.to_be_bytes())
        .context("Failed to send message length to daemon")?;

    // Then send the actual message
    stream
        .write_all(&serialized)
        .context("Failed to send message to daemon")?;

    // Now read the response
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .context("Failed to read response length from daemon")?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;

    let mut resp_buf = vec![0u8; resp_len];
    stream
        .read_exact(&mut resp_buf)
        .context("Failed to read response from daemon")?;

    let (response, consumed_len): (DaemonResponse, usize) = // decode returns tuple
        bincode::decode_from_slice(&resp_buf, config) // Use decode_from_slice
            .context("Failed to deserialize daemon response")?;

    if consumed_len != resp_len {
        eprintln!(
            "Warning: Deserialization consumed {} bytes, but message length was {}",
            consumed_len, resp_len
        );
    }

    Ok(response) // Return the extracted response
}

pub fn handle_daemon_response(response: DaemonResponse) {
    match response {
        DaemonResponse::Success(msg) => {
            println!("Success: {}", msg);
        }
    }
}
