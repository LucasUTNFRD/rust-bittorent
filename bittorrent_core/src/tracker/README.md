# BitTorrent Tracker Client

This module implements a BitTorrent tracker client that follows the official BitTorrent protocol specification. The client communicates with HTTP/HTTPS trackers to obtain peer lists and report download/upload statistics.

## Features

- **Tracker Announce Requests**: Sends properly formatted HTTP GET requests to tracker URLs, including all required parameters as specified by the protocol.
- **Response Handling**: Processes bencoded tracker responses, extracting peer information and tracker metadata.
- **Compact Response Support**: Handles both dictionary-based and compact binary peer formats.
- **Multitracker Support**: Implements BEP-0012 (Multitracker Metadata Extension) for tracker redundancy and fallback.
- **Announce Events**: Supports all standard events: started, completed, stopped, and regular announces.
- **Error Handling**: Robust error handling for various tracker response issues.

## Implementation Details

### `TrackerClient`

The main class responsible for tracker communication. It handles:

- Constructing and sending properly URL-encoded HTTP GET requests
- Processing tracker responses
- Managing tracker announcement intervals
- Implementing the tracker tier system from BEP-0012
- Tracking download/upload statistics

### Supported Announce Parameters

- `info_hash`: SHA-1 hash of the info dictionary from the torrent file (URL-encoded)
- `peer_id`: 20-byte client identifier (URL-encoded)
- `port`: Port the client is listening on for incoming connections
- `uploaded`: Total bytes uploaded
- `downloaded`: Total bytes downloaded
- `left`: Bytes left to download
- `compact`: Request for compact peer lists (more efficient)
- `event`: Type of announce (started, stopped, completed)
- `numwant`: Requested number of peers
- `trackerid`: Tracker identifier for continued communication

### Peer Information Handling

The client can parse peer information in two formats:

1. **Dictionary format**: Traditional list of peer dictionaries
2. **Compact format**: Binary string where each peer is represented by 6 bytes (4 for IP, 2 for port)

## Usage

```rust
// Create a tracker client and start the communication process
let tracker = TrackerClient::new(torrent_info, peer_id, communication_channel).start().await;

// When done, make sure to notify the tracker
tracker.stop().await;
```

## Error Handling
The `TrackerError` enum provides detailed error information for:
- HTTP request failures
- URL parsing problems
- Bencode parsing issues
- Tracker-reported errors
- Missing or invalid fields in responses

## Future Improvements
- UDP tracker protocol support (BEP-0015)
- WebSocket tracker support
