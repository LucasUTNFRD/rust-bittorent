use anyhow::{Context, Result, anyhow};
use bincode::{Decode, Encode, config}; //TODO: Implement our own serialization
use clap::{Arg, ArgAction, Command};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/bittorent-protocol.tmp";

#[derive(Debug, Encode, Decode)] // Add Encode, Decode
pub enum DaemonMsg {
    AddTorrent(String),
}

#[derive(Debug, Encode, Decode)] // Add Encode, Decode
pub enum DaemonResponse {
    Success(String),
}

fn send_daemon_message(msg: DaemonMsg) -> Result<DaemonResponse> {
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

fn handle_daemon_response(response: DaemonResponse) {
    match response {
        DaemonResponse::Success(msg) => {
            println!("Success: {}", msg);
        }
    }
}

fn main() -> Result<()> {
    let matches = Command::new("BitTorrent CLI")
        .version("0.1.0")
        .about("BitTorrent client CLI")
        .subcommand(
            Command::new("add").about("Add a torrent to download").arg(
                Arg::new("torrent-file")
                    .help("Path to the .torrent file")
                    .required(true)
                    .index(1),
            ),
        )
        .subcommand(
            Command::new("list")
                .about("List all torrents")
                .arg(
                    Arg::new("active")
                        .short('a')
                        .long("active")
                        .help("Show only active torrents")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("completed")
                        .short('c')
                        .long("completed")
                        .help("Show only completed torrents")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("status")
                .about("Get status of a torrent")
                .arg(Arg::new("id").help("Torrent ID").required(true).index(1)),
        )
        .get_matches();

    // Handle the subcommands
    let result = match matches.subcommand() {
        Some(("add", add_matches)) => {
            let torrent_path = add_matches
                .get_one::<String>("torrent-file")
                .ok_or_else(|| anyhow!("Torrent file path required"))?;

            println!("Adding torrent: {}", torrent_path);
            let msg = DaemonMsg::AddTorrent(torrent_path.clone());
            send_daemon_message(msg)
        }
        _ => {
            println!("No command provided. Try 'btcli --help' for usage information.");
            return Ok(());
        }
    };

    match result {
        Ok(response) => handle_daemon_response(response),
        Err(err) => eprintln!("Error: {}", err),
    }

    Ok(())
}
