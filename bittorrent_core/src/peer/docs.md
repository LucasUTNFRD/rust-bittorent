**BitTorrent Protocol: Message-by-Message Analysis**

Each message appears after the initial handshake. The handshake is separate and includes `info_hash` and `peer_id`. After handshake, the following messages are used:

---

### `keep-alive`

* **Expect when**: No other message has been received from a peer for 2 minutes.
* **Send when**: You haven’t sent any messages to a peer for 2 minutes.
* **Purpose**: Prevents idle TCP connections from being closed by timeouts.

---

### `choke`

* **Expect when**: The remote peer no longer wants to upload to you.
* **Send when**: You no longer want to upload to a peer (e.g., due to tit-for-tat rotation or bandwidth limits).
* **Effect**: The peer must stop sending `request` messages to you.

---

### `unchoke`

* **Expect when**: The peer is now willing to upload to you.
* **Send when**: You decide to allow a choked peer to start downloading from you again.
* **Effect**: The peer can now send `request` messages.

---

### `interested`

* **Expect when**: The peer finds that you have pieces it needs.
* **Send when**: You find that the peer has at least one piece you’re missing.
* **Effect**: Informs the remote peer that you want to download; prerequisite for being unchoked.

---

### `not interested`

* **Expect when**: The peer determines you have nothing it needs.
* **Send when**: You determine the peer has no pieces you need.
* **Effect**: Optional optimization; avoids unnecessary unchoking.

---

### `have`

* **Expect when**: The peer completes a piece.
* **Send when**: You complete a piece.
* **Effect**: Allows peers to update their internal view of your piece availability.

---

### `bitfield`

* **Expect when**: Immediately after handshake, once per connection.
* **Send when**: Immediately after handshake, if you have at least one piece.
* **Effect**: Gives a complete view of all pieces you possess at the time of connection.

---

### `request { index, begin, length }`

* **Expect when**: You are unchoked and the peer is interested.
* **Send when**: You are unchoked by the peer and interested in one of its pieces.
* **Effect**: Requests a specific block (typically 16KB) of a piece.

---

### `piece { index, begin, block }`

* **Expect when**: You have sent a `request` and the peer is unchoked and willing to upload.
* **Send when**: You are unchoked and have received a valid `request`.
* **Effect**: Delivers the requested data block.

---

### `cancel { index, begin, length }`

* **Expect when**: The peer no longer wants a previously requested block.
* **Send when**: You change plans or switch to another peer for the same piece.
* **Effect**: Informs the peer to drop a previously queued request.

---

### `port { listen_port }` *(DHT only)*

* **Expect when**: Peer uses the DHT extension and wants to tell you their DHT port.
* **Send when**: You use DHT and want to inform a peer of your UDP port.
* **Effect**: Used in DHT (Distributed Hash Table) extension for peer discovery.

---

**Message Exchange Summary (Linear Flow)**:

1. **After handshake**:

   * Receive/send `bitfield`
   * Evaluate interest
   * Send `interested` or `not interested`
2. **Peer evaluates**:

   * Sends `choke`/`unchoke`
3. **If unchoked + interested**:

   * Send `request`
   * Receive `piece`
4. **On piece completion**:

   * Send `have` to all peers

This flow repeats and adjusts as piece availability and peer decisions change.
