use anyhow::{Result, anyhow};
use clap::{Arg, ArgAction, Command};
use daemon_messages::{DaemonMsg, handle_daemon_response, send_daemon_message};

mod daemon_messages;

const SOCKET_PATH: &str = "/tmp/bittorent-protocol.tmp";

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
