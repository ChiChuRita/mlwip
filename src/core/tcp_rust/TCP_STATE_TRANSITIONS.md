# TCP State Transition Table

This document provides a comprehensive overview of TCP state transitions based on incoming segments and API calls.

## Legend

- **Event**: Incoming segment flags (SYN, ACK, FIN, RST) or API call
- **Action**: What the TCP stack does in response
- **Next State**: The resulting TCP state after the transition
- **Notes**: Additional context or validation requirements

---

## State Transition Table

### CLOSED State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| Any segment | Send RST (unless segment is RST) | CLOSED | Reject all segments |
| `tcp_bind()` | Store local IP and port | CLOSED | Prepares for listening or connecting |
| `tcp_listen()` | Start accepting connections | LISTEN | Requires prior bind |
| `tcp_connect()` | Generate ISS, prepare to send SYN | SYN_SENT | Active open |

---

### LISTEN State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| SYN (no ACK) | Store peer info, generate ISS, send SYN+ACK | SYN_RCVD | Passive open handshake |
| SYN+ACK | Send RST | LISTEN | Invalid in LISTEN |
| ACK | Send RST | LISTEN | Invalid in LISTEN |
| FIN | Send RST | LISTEN | Invalid in LISTEN |
| RST | Ignore | LISTEN | No connection to reset |

---

### SYN_SENT State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| SYN+ACK | Validate ACK=ISS+1, store IRS, send ACK | ESTABLISHED | Active open completes |
| SYN (no ACK) | Process simultaneous open | SYN_RCVD | Simultaneous open (rare) |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| Other | Drop | SYN_SENT | Invalid segment |

---

### SYN_RCVD State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK | Validate ACK=ISS+1, complete handshake | ESTABLISHED | Passive open completes |
| SYN | Retransmit SYN+ACK | SYN_RCVD | Retransmitted SYN |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| FIN | Send RST | SYN_RCVD | Invalid in handshake |

---

### ESTABLISHED State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK | Process data acknowledgment | ESTABLISHED | Normal data transfer |
| PSH+ACK | Process data, deliver to application | ESTABLISHED | Push data to app |
| FIN | ACK the FIN, notify application | CLOSE_WAIT | Passive close begins |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| SYN | Send challenge ACK | ESTABLISHED | RFC 5961 security |
| `tcp_close()` | Send FIN | FIN_WAIT_1 | Active close begins |
| `tcp_abort()` | Send RST | CLOSED | Immediate abort |

---

### FIN_WAIT_1 State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK (for our FIN) | Wait for peer's FIN | FIN_WAIT_2 | Our FIN acknowledged |
| FIN | ACK the FIN | CLOSING | Simultaneous close |
| FIN+ACK | ACK the FIN, wait | TIME_WAIT | If ACKs our FIN |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| ACK (data) | Process normally | FIN_WAIT_1 | May receive data ACKs |

---

### FIN_WAIT_2 State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| FIN | ACK the FIN, start TIME_WAIT timer | TIME_WAIT | Waiting for delayed segments |
| ACK | Process data acknowledgment | FIN_WAIT_2 | May still receive ACKs |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |

---

### CLOSE_WAIT State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK | Process acknowledgment | CLOSE_WAIT | May receive ACKs for sent data |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| `tcp_close()` | Send FIN | LAST_ACK | Application closes connection |

---

### CLOSING State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK (for our FIN) | Start TIME_WAIT timer | TIME_WAIT | Simultaneous close completes |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| Other | Drop or process | CLOSING | Wait for FIN ACK |

---

### LAST_ACK State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| ACK (for our FIN) | Close connection | CLOSED | Passive close completes |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| Other | Drop | LAST_ACK | Waiting only for final ACK |

---

### TIME_WAIT State

| Event | Action | Next State | Notes |
|-------|--------|------------|-------|
| FIN | Re-ACK the FIN | TIME_WAIT | Handle retransmitted FIN |
| RST | Abort connection | CLOSED | RFC 5961 validation applied |
| Timeout (2MSL) | Close connection | CLOSED | Normal completion after 2MSL |
| Other | Ignore | TIME_WAIT | Wait for timer expiration |

---

## State Transition Diagram

```
Active Open (tcp_connect):
CLOSED → SYN_SENT → ESTABLISHED

Passive Open (tcp_listen):
CLOSED → LISTEN → SYN_RCVD → ESTABLISHED

Active Close from ESTABLISHED:
ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 → TIME_WAIT → CLOSED

Passive Close from ESTABLISHED:
ESTABLISHED → CLOSE_WAIT → LAST_ACK → CLOSED

Simultaneous Close:
ESTABLISHED → FIN_WAIT_1 → CLOSING → TIME_WAIT → CLOSED

Abort (RST or tcp_abort):
ANY_STATE → CLOSED
```

---

## Validation Rules (RFC 5961)

### Sequence Number Validation

All states except CLOSED and LISTEN validate incoming sequence numbers:

- **Zero Window**: `SEG.SEQ == RCV.NXT`
- **Non-Zero Window**: `RCV.NXT ≤ SEG.SEQ < RCV.NXT + RCV.WND` OR segment overlaps window

Invalid sequence numbers result in:
- **Drop** the segment
- Potentially send **Challenge ACK** (for RST or SYN)

### ACK Validation

Valid ACK must satisfy:
- `SND.UNA < SEG.ACK ≤ SND.NXT`

ACK validation results:
- **Valid**: `SND.UNA < SEG.ACK ≤ SND.NXT` - Process normally
- **Duplicate**: `SEG.ACK == SND.UNA` - May trigger fast retransmit
- **Future**: `SEG.ACK > SND.NXT` - Send Challenge ACK (RFC 5961)
- **Old**: `SEG.ACK < SND.UNA` - Drop segment

### RST Validation

RST validation (RFC 5961):
- **Valid**: Sequence number in receive window - Accept RST, transition to CLOSED
- **Challenge**: Sequence number outside window - Send Challenge ACK, ignore RST
- **Invalid**: Drop segment

---

## Key Implementation Notes

1. **SYN Consumes One Sequence Number**: When processing SYN, `rcv_nxt = seg.seqno + 1`

2. **FIN Consumes One Sequence Number**: When processing FIN, `rcv_nxt = rcv_nxt + 1`

3. **ACK for FIN**: To ACK a FIN, the ACK number should be `peer_fin_seq + 1`

4. **ISS Generation**: Initial Sequence Number should be generated securely (RFC 6528)

5. **TIME_WAIT Duration**: 2MSL (Maximum Segment Lifetime), typically 2-4 minutes

6. **Challenge ACK Rate Limiting**: RFC 5961 recommends rate-limiting challenge ACKs

---

## Implementation Status

✅ **Implemented States:**
- CLOSED, LISTEN, SYN_SENT, SYN_RCVD, ESTABLISHED
- FIN_WAIT_1, FIN_WAIT_2, CLOSE_WAIT, CLOSING, LAST_ACK, TIME_WAIT

✅ **Implemented Transitions:**
- 3-way handshake (active and passive)
- Connection termination (active, passive, and simultaneous close)
- RST handling with RFC 5961 validation
- Sequence number and ACK validation

🚧 **Partial Implementation:**
- Simultaneous open (SYN in SYN_SENT)
- Retransmission handling
- TIME_WAIT timer expiration

---

## Testing Coverage

See `tests/handshake_tests.rs` and `tests/control_path_tests.rs` for comprehensive tests covering:
- ✅ Passive open handshake
- ✅ Active open handshake
- ✅ Active close
- ✅ Passive close
- ✅ Simultaneous close
- ✅ RST handling
- ✅ Sequence number validation
- ✅ ACK validation (RFC 5961)

---

## References

- **RFC 793**: Transmission Control Protocol
- **RFC 5961**: Improving TCP's Robustness to Blind In-Window Attacks
- **RFC 6528**: Defending against Sequence Number Attacks
- **RFC 7323**: TCP Extensions for High Performance
