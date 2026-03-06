//! Event types and handlers for the intercom system.

use intercom_protocol::{ChannelId, ChannelInfo, RoomId, UserId, UserInfo};
use intercom_transport::ConnectionState;

/// Events emitted by the intercom system.
#[derive(Debug, Clone)]
pub enum IntercomEvent {
    // Connection events
    /// Connection state changed.
    ConnectionStateChanged(ConnectionState),
    /// Successfully connected to server.
    Connected { room_id: RoomId, user_id: UserId },
    /// Disconnected from server.
    Disconnected { reason: String },
    /// Connection error occurred.
    ConnectionError { error: String },

    // Room events
    /// Joined a room.
    RoomJoined { room_id: RoomId, room_name: String },
    /// Left a room.
    RoomLeft { room_id: RoomId },
    /// Room state updated.
    RoomUpdated {
        channels: Vec<ChannelInfo>,
        users: Vec<UserInfo>,
    },

    // User events
    /// User joined the room.
    UserJoined(UserInfo),
    /// User left the room.
    UserLeft(UserId),
    /// User started talking.
    UserTalkStart { user_id: UserId, channel_id: ChannelId },
    /// User stopped talking.
    UserTalkStop { user_id: UserId },

    // Channel events
    /// Subscribed to a channel.
    ChannelSubscribed(ChannelId),
    /// Unsubscribed from a channel.
    ChannelUnsubscribed(ChannelId),
    /// Channel created.
    ChannelCreated(ChannelInfo),
    /// Channel deleted.
    ChannelDeleted(ChannelId),

    // Audio events
    /// Local talk started.
    TalkStarted { channel_id: ChannelId },
    /// Local talk stopped.
    TalkStopped,
    /// Audio level update (for VU meters).
    AudioLevel {
        input_level: f32,
        output_level: f32,
    },
    /// Voice activity detected.
    VoiceActivity { active: bool },

    // Error events
    /// General error.
    Error { code: u32, message: String },
}

/// Trait for handling intercom events.
pub trait EventHandler: Send + Sync {
    /// Called when an event occurs.
    fn on_event(&self, event: IntercomEvent);
}

/// Simple callback-based event handler.
pub struct CallbackEventHandler {
    callback: Box<dyn Fn(IntercomEvent) + Send + Sync>,
}

impl CallbackEventHandler {
    /// Create a new callback event handler.
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(IntercomEvent) + Send + Sync + 'static,
    {
        Self {
            callback: Box::new(callback),
        }
    }
}

impl EventHandler for CallbackEventHandler {
    fn on_event(&self, event: IntercomEvent) {
        (self.callback)(event);
    }
}

/// Multi-handler that dispatches to multiple handlers.
pub struct MultiEventHandler {
    handlers: Vec<Box<dyn EventHandler>>,
}

impl MultiEventHandler {
    /// Create a new multi-handler.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Add a handler.
    pub fn add<H: EventHandler + 'static>(&mut self, handler: H) {
        self.handlers.push(Box::new(handler));
    }
}

impl Default for MultiEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler for MultiEventHandler {
    fn on_event(&self, event: IntercomEvent) {
        for handler in &self.handlers {
            handler.on_event(event.clone());
        }
    }
}

/// Async event channel for receiving events.
pub struct EventChannel {
    sender: crossbeam_channel::Sender<IntercomEvent>,
    receiver: crossbeam_channel::Receiver<IntercomEvent>,
}

impl EventChannel {
    /// Create a new event channel.
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(256);
        Self { sender, receiver }
    }

    /// Get the sender for use with the event handler.
    pub fn handler(&self) -> ChannelEventHandler {
        ChannelEventHandler {
            sender: self.sender.clone(),
        }
    }

    /// Receive the next event (blocking).
    pub fn recv(&self) -> Option<IntercomEvent> {
        self.receiver.recv().ok()
    }

    /// Try to receive an event (non-blocking).
    pub fn try_recv(&self) -> Option<IntercomEvent> {
        self.receiver.try_recv().ok()
    }

    /// Get the receiver for iteration.
    pub fn receiver(&self) -> &crossbeam_channel::Receiver<IntercomEvent> {
        &self.receiver
    }
}

impl Default for EventChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Event handler that sends to a channel.
pub struct ChannelEventHandler {
    sender: crossbeam_channel::Sender<IntercomEvent>,
}

impl EventHandler for ChannelEventHandler {
    fn on_event(&self, event: IntercomEvent) {
        let _ = self.sender.try_send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_callback_handler() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let handler = CallbackEventHandler::new(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        handler.on_event(IntercomEvent::TalkStopped);
        handler.on_event(IntercomEvent::TalkStopped);

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_channel() {
        let channel = EventChannel::new();
        let handler = channel.handler();

        handler.on_event(IntercomEvent::TalkStopped);
        handler.on_event(IntercomEvent::VoiceActivity { active: true });

        let event1 = channel.try_recv().unwrap();
        assert!(matches!(event1, IntercomEvent::TalkStopped));

        let event2 = channel.try_recv().unwrap();
        assert!(matches!(event2, IntercomEvent::VoiceActivity { active: true }));
    }
}
