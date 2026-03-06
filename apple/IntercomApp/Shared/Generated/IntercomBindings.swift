// PLACEHOLDER: This file will be replaced by generated UniFFI bindings
// Run: ./build-xcframework.sh to generate actual bindings

import Foundation

// Temporary stubs until bindings are generated

public struct AudioDeviceInfo: Identifiable {
    public var id: String { name }
    public let name: String
    public let isDante: Bool
    public let isDefault: Bool
}

public struct ClientConfiguration {
    public var displayName: String
    public var inputDevice: String?
    public var outputDevice: String?
    public var firebaseUrl: String
    public var pushToTalk: Bool
    public var preferDante: Bool

    public init(displayName: String, inputDevice: String? = nil, outputDevice: String? = nil,
                firebaseUrl: String, pushToTalk: Bool = true, preferDante: Bool = true) {
        self.displayName = displayName
        self.inputDevice = inputDevice
        self.outputDevice = outputDevice
        self.firebaseUrl = firebaseUrl
        self.pushToTalk = pushToTalk
        self.preferDante = preferDante
    }
}

public struct ServerConfiguration {
    public var name: String
    public var password: String?
    public var firebaseUrl: String
    public var maxUsers: UInt32
    public var defaultChannels: [String]
    public var preferDante: Bool

    public init(name: String, password: String? = nil, firebaseUrl: String,
                maxUsers: UInt32 = 100, defaultChannels: [String] = [], preferDante: Bool = true) {
        self.name = name
        self.password = password
        self.firebaseUrl = firebaseUrl
        self.maxUsers = maxUsers
        self.defaultChannels = defaultChannels
        self.preferDante = preferDante
    }
}

public struct ChannelInfo: Identifiable {
    public let id: String
    public let name: String
    public let description: String?
}

public struct UserInfo: Identifiable {
    public let id: String
    public let displayName: String
    public let isTalking: Bool
}

public enum ConnectionState {
    case new
    case connecting
    case connected
    case disconnected
    case failed
    case closed
}

public enum IntercomEvent {
    case connectionStateChanged(state: ConnectionState)
    case connected(roomId: String, userId: String)
    case disconnected(reason: String)
    case userJoined(user: UserInfo)
    case userLeft(userId: String)
    case userTalkStart(userId: String, channelId: String)
    case userTalkStop(userId: String)
    case channelCreated(channel: ChannelInfo)
    case channelDeleted(channelId: String)
    case talkStarted(channelId: String)
    case talkStopped
    case audioLevel(inputLevel: Float, outputLevel: Float)
    case error(message: String)
}

public enum IntercomError: Error {
    case audioError(message: String)
    case connectionError(message: String)
    case signalingError(message: String)
    case notConnected
    case alreadyConnected
    case invalidConfig(message: String)
    case timeout
    case internalError(message: String)
}

// Stub functions - replace with actual FFI calls
public func getVersion() -> String {
    return "0.1.0 (stub)"
}

public func listInputDevices() -> [AudioDeviceInfo] {
    // Stub: return empty list until FFI is connected
    return []
}

public func listOutputDevices() -> [AudioDeviceInfo] {
    // Stub: return empty list until FFI is connected
    return []
}

// Stub client class
public class IntercomClient {
    public init(config: ClientConfiguration) throws {
        // Stub
    }

    public func getNextEvent() -> IntercomEvent? {
        return nil
    }

    public func getPendingEvents() -> [IntercomEvent] {
        return []
    }

    public func getUserId() -> String {
        return "stub-user-id"
    }

    public func getRoomId() -> String? {
        return nil
    }

    public func isConnected() -> Bool {
        return false
    }

    public func initAudio() throws {
        // Stub
    }

    public func setInputDevice(deviceName: String) throws {
        // Stub
    }

    public func setOutputDevice(deviceName: String) throws {
        // Stub
    }

    public func connect(roomId: String) throws {
        // Stub
    }

    public func disconnect() throws {
        // Stub
    }

    public func subscribeChannel(channelId: String) {
        // Stub
    }

    public func unsubscribeChannel(channelId: String) {
        // Stub
    }

    public func startTalk(channelId: String) throws {
        // Stub
    }

    public func stopTalk() throws {
        // Stub
    }

    public func isTalking() -> Bool {
        return false
    }

    public func getTalkChannel() -> String? {
        return nil
    }

    public func startAudio() throws {
        // Stub
    }

    public func stopAudio() throws {
        // Stub
    }
}

// Stub server class
public class IntercomServer {
    public init(config: ServerConfiguration) throws {
        // Stub
    }

    public func getNextEvent() -> IntercomEvent? {
        return nil
    }

    public func getPendingEvents() -> [IntercomEvent] {
        return []
    }

    public func getRoomId() -> String {
        return "stub-room-id"
    }

    public func getEncryptionKey() -> [UInt8] {
        return []
    }

    public func isRunning() -> Bool {
        return false
    }

    public func initAudio() throws {
        // Stub
    }

    public func start() throws {
        // Stub
    }

    public func stop() throws {
        // Stub
    }

    public func getUsers() -> [UserInfo] {
        return []
    }

    public func getChannels() -> [ChannelInfo] {
        return []
    }

    public func createChannel(name: String) throws -> String {
        return "stub-channel-id"
    }

    public func deleteChannel(channelId: String) throws {
        // Stub
    }

    public func kickUser(userId: String) throws {
        // Stub
    }
}
