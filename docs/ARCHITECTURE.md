# Architecture Overview

## System Design

```
┌─────────────────────────────────────────────────────────────┐
│                    INTERCOM SYSTEM                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    RUST CORE                         │    │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐    │    │
│  │  │   Audio     │ │   WebRTC    │ │  Protocol   │    │    │
│  │  │   Engine    │ │   + ICE     │ │   Layer     │    │    │
│  │  │   (cpal)    │ │ (webrtc-rs) │ │  (serde)    │    │    │
│  │  └─────────────┘ └─────────────┘ └─────────────┘    │    │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐    │    │
│  │  │   Opus      │ │   Channel   │ │   Session   │    │    │
│  │  │   Codec     │ │   Mixer     │ │   Manager   │    │    │
│  │  │ (audiopus)  │ │             │ │             │    │    │
│  │  └─────────────┘ └─────────────┘ └─────────────┘    │    │
│  └─────────────────────────────────────────────────────┘    │
│                            │                                 │
│                      FFI (UniFFI)                            │
│         ┌──────────────────┼──────────────────┐             │
│         ▼                  ▼                  ▼             │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐       │
│  │   Swift     │   │   Swift     │   │    C++/Qt   │       │
│  │   macOS     │   │    iOS      │   │   Windows   │       │
│  └─────────────┘   └─────────────┘   └─────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

## Crate Structure

### intercom-protocol
Core protocol definitions shared across all components:
- Message types (ClientMessage, ServerMessage)
- Data structures (UserId, ChannelId, RoomId)
- Audio packet format
- Serialization (bincode for binary, JSON for signaling)

### intercom-audio
Audio I/O layer using cpal:
- Cross-platform audio capture/playback
- Device enumeration with DANTE priority
- Resampling for device compatibility
- Voice Activity Detection (VAD)

### intercom-codec
Opus codec wrapper:
- Low-latency voice encoding (10ms frames)
- 48kHz mono at 32-64kbps
- Forward Error Correction (FEC)
- Packet Loss Concealment (PLC)

### intercom-mixer
Audio mixing and buffering:
- Multi-user channel mixing
- Adaptive jitter buffer (20-100ms)
- Per-user gain and mute
- Soft clipping for headroom

### intercom-transport
WebRTC networking:
- Peer-to-peer connections via webrtc-rs
- ICE/STUN for NAT traversal
- Unreliable data channels for audio
- Connection state management

### intercom-signaling
Firebase REST signaling:
- Offer/Answer exchange
- ICE candidate relay
- Presence management
- No server deployment required

### intercom-crypto
Encryption layer:
- AES-256-GCM authenticated encryption
- Per-session keys
- Constant-time operations

### intercom-session
State management:
- Room/channel/user tracking
- Subscription management
- Talk state coordination

### intercom-core
Main facade combining all components:
- IntercomClient for end users
- IntercomServer for room hosting
- Event system for UI updates

### intercom-ffi
Foreign Function Interface:
- UniFFI for Swift bindings
- cbindgen for C++ headers

## Audio Pipeline

```
Capture → VAD → Opus Encode → Encrypt → WebRTC Send
                                            ↓
Playback ← Mix ← Jitter Buffer ← Opus Decode ← Decrypt ← WebRTC Receive
```

### Audio Parameters
| Parameter | Value |
|-----------|-------|
| Sample Rate | 48000 Hz |
| Channels | 1 (mono) |
| Frame Size | 480 samples (10ms) |
| Opus Bitrate | 32-64 kbps |
| Jitter Buffer | 20-60ms adaptive |

## NAT Traversal

### STUN Servers (Public)
- stun:stun.l.google.com:19302
- stun:stun1.l.google.com:19302
- stun:stun.cloudflare.com:3478

### Signaling via Firebase
- Free tier (100 concurrent connections)
- REST API for simplicity
- Real-time updates via polling (500ms)

### Connection Flow
1. Client A joins room via Firebase signaling
2. Client A creates WebRTC offer
3. Offer sent to Client B via Firebase inbox
4. Client B creates answer, sends back
5. ICE candidates exchanged
6. Direct P2P connection established
7. Audio flows directly between peers

## Security

### Encryption
- All audio encrypted with AES-256-GCM
- Room key shared via signaling (can be password-derived)
- Each packet has unique nonce

### Authentication
- Firebase Auth integration (optional)
- Room passwords for access control
- Per-user permissions possible

## Platform Notes

### macOS
- Uses AVFoundation for permissions
- Menu bar for quick access (future)
- Global hotkey support

### iOS
- AVAudioSession for background audio
- Lock screen controls
- Push-to-talk with haptic feedback

### Windows
- WASAPI via cpal for low latency
- System tray integration (future)
- Global hotkey support

### DANTE Support
- Detected as virtual soundcard
- Requires DANTE Virtual Soundcard installed
- Priority selection in device settings
- Works with existing DANTE infrastructure
