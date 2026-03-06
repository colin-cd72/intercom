//! Session and user state management for the intercom system.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use intercom_protocol::{ChannelId, ChannelInfo, RoomId, UserId, UserInfo};

/// Session errors.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("Already in room")]
    AlreadyInRoom,

    #[error("Not in room")]
    NotInRoom,

    #[error("Permission denied")]
    PermissionDenied,
}

/// User connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserState {
    Connecting,
    Connected,
    Talking,
    Idle,
    Disconnected,
}

/// Extended user state for session management.
#[derive(Debug, Clone)]
pub struct UserSession {
    pub info: UserInfo,
    pub state: UserState,
    pub joined_at: Instant,
    pub last_activity: Instant,
    pub subscribed_channels: HashSet<ChannelId>,
    pub talk_channel: Option<ChannelId>,
    pub volume: f32,
    pub muted: bool,
}

impl UserSession {
    pub fn new(info: UserInfo) -> Self {
        Self {
            info,
            state: UserState::Connected,
            joined_at: Instant::now(),
            last_activity: Instant::now(),
            subscribed_channels: HashSet::new(),
            talk_channel: None,
            volume: 1.0,
            muted: false,
        }
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

/// Channel state for session management.
#[derive(Debug, Clone)]
pub struct ChannelSession {
    pub info: ChannelInfo,
    pub subscribers: HashSet<UserId>,
    pub talkers: HashSet<UserId>,
}

impl ChannelSession {
    pub fn new(info: ChannelInfo) -> Self {
        Self {
            info,
            subscribers: HashSet::new(),
            talkers: HashSet::new(),
        }
    }
}

/// Room session state.
#[derive(Debug)]
pub struct RoomSession {
    pub id: RoomId,
    pub name: String,
    pub password_hash: Option<String>,
    pub users: HashMap<UserId, UserSession>,
    pub channels: HashMap<ChannelId, ChannelSession>,
    pub created_at: Instant,
    pub max_users: usize,
}

impl RoomSession {
    pub fn new(id: RoomId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            password_hash: None,
            users: HashMap::new(),
            channels: HashMap::new(),
            created_at: Instant::now(),
            max_users: 100,
        }
    }

    pub fn with_password(mut self, password_hash: String) -> Self {
        self.password_hash = Some(password_hash);
        self
    }

    pub fn with_max_users(mut self, max: usize) -> Self {
        self.max_users = max;
        self
    }
}

/// Session manager for tracking all state.
pub struct SessionManager {
    rooms: RwLock<HashMap<RoomId, RoomSession>>,
    user_room_map: RwLock<HashMap<UserId, RoomId>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            user_room_map: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new room.
    pub fn create_room(&self, room: RoomSession) -> RoomId {
        let id = room.id.clone();
        self.rooms.write().insert(id.clone(), room);
        info!("Created room: {}", id);
        id
    }

    /// Delete a room.
    pub fn delete_room(&self, room_id: &RoomId) -> Result<(), SessionError> {
        let room = self
            .rooms
            .write()
            .remove(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        // Remove user mappings
        let mut user_map = self.user_room_map.write();
        for user_id in room.users.keys() {
            user_map.remove(user_id);
        }

        info!("Deleted room: {}", room_id);
        Ok(())
    }

    /// Get room info.
    pub fn get_room(&self, room_id: &RoomId) -> Option<(String, usize, usize)> {
        self.rooms.read().get(room_id).map(|r| {
            (r.name.clone(), r.users.len(), r.max_users)
        })
    }

    /// List all rooms.
    pub fn list_rooms(&self) -> Vec<RoomId> {
        self.rooms.read().keys().cloned().collect()
    }

    /// Add a user to a room.
    pub fn join_room(
        &self,
        room_id: &RoomId,
        user: UserInfo,
    ) -> Result<(), SessionError> {
        let user_id = user.id.clone();

        // Check if already in a room
        if self.user_room_map.read().contains_key(&user_id) {
            return Err(SessionError::AlreadyInRoom);
        }

        let mut rooms = self.rooms.write();
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        if room.users.len() >= room.max_users {
            return Err(SessionError::PermissionDenied);
        }

        room.users.insert(user_id.clone(), UserSession::new(user));
        self.user_room_map.write().insert(user_id.clone(), room_id.clone());

        info!("User {} joined room {}", user_id, room_id);
        Ok(())
    }

    /// Remove a user from their room.
    pub fn leave_room(&self, user_id: &UserId) -> Result<RoomId, SessionError> {
        let room_id = self
            .user_room_map
            .write()
            .remove(user_id)
            .ok_or_else(|| SessionError::NotInRoom)?;

        let mut rooms = self.rooms.write();
        if let Some(room) = rooms.get_mut(&room_id) {
            // Remove from all channels
            for channel in room.channels.values_mut() {
                channel.subscribers.remove(user_id);
                channel.talkers.remove(user_id);
            }
            room.users.remove(user_id);
        }

        info!("User {} left room {}", user_id, room_id);
        Ok(room_id)
    }

    /// Get user's current room.
    pub fn get_user_room(&self, user_id: &UserId) -> Option<RoomId> {
        self.user_room_map.read().get(user_id).cloned()
    }

    /// Create a channel in a room.
    pub fn create_channel(
        &self,
        room_id: &RoomId,
        channel: ChannelInfo,
    ) -> Result<ChannelId, SessionError> {
        let channel_id = channel.id.clone();

        let mut rooms = self.rooms.write();
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        room.channels.insert(channel_id.clone(), ChannelSession::new(channel));

        info!("Created channel {} in room {}", channel_id, room_id);
        Ok(channel_id)
    }

    /// Delete a channel from a room.
    pub fn delete_channel(
        &self,
        room_id: &RoomId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        let mut rooms = self.rooms.write();
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        room.channels
            .remove(channel_id)
            .ok_or_else(|| SessionError::ChannelNotFound(channel_id.to_string()))?;

        // Remove from user subscriptions
        for user in room.users.values_mut() {
            user.subscribed_channels.remove(channel_id);
            if user.talk_channel.as_ref() == Some(channel_id) {
                user.talk_channel = None;
                user.state = UserState::Idle;
            }
        }

        info!("Deleted channel {} from room {}", channel_id, room_id);
        Ok(())
    }

    /// Subscribe a user to a channel.
    pub fn subscribe_channel(
        &self,
        user_id: &UserId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        let room_id = self
            .user_room_map
            .read()
            .get(user_id)
            .cloned()
            .ok_or_else(|| SessionError::NotInRoom)?;

        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(&room_id).unwrap();

        let channel = room
            .channels
            .get_mut(channel_id)
            .ok_or_else(|| SessionError::ChannelNotFound(channel_id.to_string()))?;

        channel.subscribers.insert(user_id.clone());

        if let Some(user) = room.users.get_mut(user_id) {
            user.subscribed_channels.insert(channel_id.clone());
            user.touch();
        }

        debug!("User {} subscribed to channel {}", user_id, channel_id);
        Ok(())
    }

    /// Unsubscribe a user from a channel.
    pub fn unsubscribe_channel(
        &self,
        user_id: &UserId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        let room_id = self
            .user_room_map
            .read()
            .get(user_id)
            .cloned()
            .ok_or_else(|| SessionError::NotInRoom)?;

        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(&room_id).unwrap();

        if let Some(channel) = room.channels.get_mut(channel_id) {
            channel.subscribers.remove(user_id);
            channel.talkers.remove(user_id);
        }

        if let Some(user) = room.users.get_mut(user_id) {
            user.subscribed_channels.remove(channel_id);
            if user.talk_channel.as_ref() == Some(channel_id) {
                user.talk_channel = None;
                user.state = UserState::Idle;
            }
            user.touch();
        }

        debug!("User {} unsubscribed from channel {}", user_id, channel_id);
        Ok(())
    }

    /// Start talking on a channel.
    pub fn start_talk(
        &self,
        user_id: &UserId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        let room_id = self
            .user_room_map
            .read()
            .get(user_id)
            .cloned()
            .ok_or_else(|| SessionError::NotInRoom)?;

        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(&room_id).unwrap();

        let channel = room
            .channels
            .get_mut(channel_id)
            .ok_or_else(|| SessionError::ChannelNotFound(channel_id.to_string()))?;

        channel.talkers.insert(user_id.clone());

        if let Some(user) = room.users.get_mut(user_id) {
            user.talk_channel = Some(channel_id.clone());
            user.state = UserState::Talking;
            user.touch();
        }

        debug!("User {} started talking on channel {}", user_id, channel_id);
        Ok(())
    }

    /// Stop talking.
    pub fn stop_talk(&self, user_id: &UserId) -> Result<Option<ChannelId>, SessionError> {
        let room_id = self
            .user_room_map
            .read()
            .get(user_id)
            .cloned()
            .ok_or_else(|| SessionError::NotInRoom)?;

        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(&room_id).unwrap();

        let channel_id = if let Some(user) = room.users.get_mut(user_id) {
            let ch = user.talk_channel.take();
            user.state = UserState::Idle;
            user.touch();
            ch
        } else {
            None
        };

        if let Some(ref ch_id) = channel_id {
            if let Some(channel) = room.channels.get_mut(ch_id) {
                channel.talkers.remove(user_id);
            }
        }

        if channel_id.is_some() {
            debug!("User {} stopped talking", user_id);
        }
        Ok(channel_id)
    }

    /// Get users in a room.
    pub fn get_room_users(&self, room_id: &RoomId) -> Result<Vec<UserInfo>, SessionError> {
        let rooms = self.rooms.read();
        let room = rooms
            .get(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        Ok(room.users.values().map(|u| u.info.clone()).collect())
    }

    /// Get channels in a room.
    pub fn get_room_channels(&self, room_id: &RoomId) -> Result<Vec<ChannelInfo>, SessionError> {
        let rooms = self.rooms.read();
        let room = rooms
            .get(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        Ok(room.channels.values().map(|c| c.info.clone()).collect())
    }

    /// Get subscribers of a channel.
    pub fn get_channel_subscribers(
        &self,
        room_id: &RoomId,
        channel_id: &ChannelId,
    ) -> Result<Vec<UserId>, SessionError> {
        let rooms = self.rooms.read();
        let room = rooms
            .get(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        let channel = room
            .channels
            .get(channel_id)
            .ok_or_else(|| SessionError::ChannelNotFound(channel_id.to_string()))?;

        Ok(channel.subscribers.iter().cloned().collect())
    }

    /// Get current talkers on a channel.
    pub fn get_channel_talkers(
        &self,
        room_id: &RoomId,
        channel_id: &ChannelId,
    ) -> Result<Vec<UserId>, SessionError> {
        let rooms = self.rooms.read();
        let room = rooms
            .get(room_id)
            .ok_or_else(|| SessionError::RoomNotFound(room_id.to_string()))?;

        let channel = room
            .channels
            .get(channel_id)
            .ok_or_else(|| SessionError::ChannelNotFound(channel_id.to_string()))?;

        Ok(channel.talkers.iter().cloned().collect())
    }

    /// Clean up idle users.
    pub fn cleanup_idle_users(&self, max_idle: Duration) -> Vec<(RoomId, UserId)> {
        let mut removed = Vec::new();

        let mut rooms = self.rooms.write();
        let mut user_map = self.user_room_map.write();

        for (room_id, room) in rooms.iter_mut() {
            let idle_users: Vec<UserId> = room
                .users
                .iter()
                .filter(|(_, u)| u.idle_duration() > max_idle)
                .map(|(id, _)| id.clone())
                .collect();

            for user_id in idle_users {
                room.users.remove(&user_id);
                user_map.remove(&user_id);

                for channel in room.channels.values_mut() {
                    channel.subscribers.remove(&user_id);
                    channel.talkers.remove(&user_id);
                }

                warn!("Removed idle user {} from room {}", user_id, room_id);
                removed.push((room_id.clone(), user_id));
            }
        }

        removed
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_room() {
        let manager = SessionManager::new();
        let room = RoomSession::new(RoomId::new("test"), "Test Room");
        let id = manager.create_room(room);
        assert_eq!(id.as_str(), "test");
        assert!(manager.list_rooms().contains(&id));
    }

    #[test]
    fn test_join_leave_room() {
        let manager = SessionManager::new();
        let room = RoomSession::new(RoomId::new("test"), "Test Room");
        let room_id = manager.create_room(room);

        let user = UserInfo::new(UserId::new("user1"), "User 1");
        manager.join_room(&room_id, user).unwrap();

        assert!(manager.get_user_room(&UserId::new("user1")).is_some());

        manager.leave_room(&UserId::new("user1")).unwrap();
        assert!(manager.get_user_room(&UserId::new("user1")).is_none());
    }

    #[test]
    fn test_channel_subscription() {
        let manager = SessionManager::new();
        let room = RoomSession::new(RoomId::new("test"), "Test Room");
        let room_id = manager.create_room(room);

        let channel = ChannelInfo::new(ChannelId::new("ch1"), "Channel 1");
        manager.create_channel(&room_id, channel).unwrap();

        let user = UserInfo::new(UserId::new("user1"), "User 1");
        manager.join_room(&room_id, user).unwrap();

        manager
            .subscribe_channel(&UserId::new("user1"), &ChannelId::new("ch1"))
            .unwrap();

        let subs = manager
            .get_channel_subscribers(&room_id, &ChannelId::new("ch1"))
            .unwrap();
        assert!(subs.contains(&UserId::new("user1")));
    }

    #[test]
    fn test_talk_state() {
        let manager = SessionManager::new();
        let room = RoomSession::new(RoomId::new("test"), "Test Room");
        let room_id = manager.create_room(room);

        let channel = ChannelInfo::new(ChannelId::new("ch1"), "Channel 1");
        manager.create_channel(&room_id, channel).unwrap();

        let user = UserInfo::new(UserId::new("user1"), "User 1");
        manager.join_room(&room_id, user).unwrap();

        manager
            .start_talk(&UserId::new("user1"), &ChannelId::new("ch1"))
            .unwrap();

        let talkers = manager
            .get_channel_talkers(&room_id, &ChannelId::new("ch1"))
            .unwrap();
        assert!(talkers.contains(&UserId::new("user1")));

        manager.stop_talk(&UserId::new("user1")).unwrap();

        let talkers = manager
            .get_channel_talkers(&room_id, &ChannelId::new("ch1"))
            .unwrap();
        assert!(!talkers.contains(&UserId::new("user1")));
    }
}
