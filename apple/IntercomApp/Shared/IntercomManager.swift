import Foundation
import Combine

/// Main intercom manager that wraps the Rust FFI client/server
@MainActor
public class IntercomManager: ObservableObject {

    // MARK: - Published State

    @Published public private(set) var connectionState: ConnectionState = .disconnected
    @Published public private(set) var isConnected: Bool = false
    @Published public private(set) var isTalking: Bool = false
    @Published public private(set) var currentRoomId: String?
    @Published public private(set) var currentChannel: String?
    @Published public private(set) var users: [UserInfo] = []
    @Published public private(set) var channels: [ChannelInfo] = []
    @Published public private(set) var inputLevel: Float = 0
    @Published public private(set) var outputLevel: Float = 0
    @Published public private(set) var errorMessage: String?

    // MARK: - Device State

    @Published public private(set) var inputDevices: [AudioDeviceInfo] = []
    @Published public private(set) var outputDevices: [AudioDeviceInfo] = []
    @Published public var selectedInputDevice: String?
    @Published public var selectedOutputDevice: String?
    @Published public var preferDante: Bool = true

    // MARK: - Private Properties

    private var client: IntercomClient?
    private var server: IntercomServer?
    private var eventCancellables = Set<AnyCancellable>()

    // MARK: - Initialization

    public init() {
        refreshDevices()
    }

    // MARK: - Device Management

    public func refreshDevices() {
        // Note: These will call the Rust FFI when integrated
        // For now, using placeholder implementation
        inputDevices = []
        outputDevices = []

        #if DEBUG
        // Mock devices for development
        inputDevices = [
            AudioDeviceInfo(name: "Dante Virtual Soundcard", isDante: true, isDefault: false),
            AudioDeviceInfo(name: "Built-in Microphone", isDante: false, isDefault: true),
            AudioDeviceInfo(name: "USB Microphone", isDante: false, isDefault: false)
        ]
        outputDevices = [
            AudioDeviceInfo(name: "Dante Virtual Soundcard", isDante: true, isDefault: false),
            AudioDeviceInfo(name: "Built-in Output", isDante: false, isDefault: true),
            AudioDeviceInfo(name: "External Speakers", isDante: false, isDefault: false)
        ]
        #endif

        // Auto-select DANTE if preferred
        if preferDante {
            if let danteInput = inputDevices.first(where: { $0.isDante }) {
                selectedInputDevice = danteInput.name
            }
            if let danteOutput = outputDevices.first(where: { $0.isDante }) {
                selectedOutputDevice = danteOutput.name
            }
        }
    }

    public func setInputDevice(_ name: String) {
        selectedInputDevice = name
        // TODO: Call Rust FFI to set device
    }

    public func setOutputDevice(_ name: String) {
        selectedOutputDevice = name
        // TODO: Call Rust FFI to set device
    }

    // MARK: - Connection Management

    public func connect(roomId: String, displayName: String) async throws {
        connectionState = .connecting

        // TODO: Create and configure IntercomClient via FFI
        // let config = ClientConfiguration(
        //     displayName: displayName,
        //     inputDevice: selectedInputDevice,
        //     outputDevice: selectedOutputDevice,
        //     firebaseUrl: "https://your-project.firebaseio.com",
        //     pushToTalk: true,
        //     preferDante: preferDante
        // )
        // client = try IntercomClient(config: config)
        // try client?.connect(roomId: roomId)

        // Simulated connection for now
        try await Task.sleep(nanoseconds: 500_000_000)

        currentRoomId = roomId
        connectionState = .connected
        isConnected = true
    }

    public func disconnect() async {
        connectionState = .disconnecting

        // TODO: Call Rust FFI to disconnect
        // try? client?.disconnect()

        currentRoomId = nil
        currentChannel = nil
        isTalking = false
        connectionState = .disconnected
        isConnected = false
        users = []
    }

    // MARK: - Server Mode

    public func startServer(name: String, password: String? = nil) async throws -> String {
        // TODO: Create and configure IntercomServer via FFI
        // let config = ServerConfiguration(...)
        // server = try IntercomServer(config: config)
        // try server?.start()
        // return server?.getRoomId() ?? ""

        return "mock-room-id"
    }

    public func stopServer() async {
        // TODO: Call Rust FFI to stop server
        // try? server?.stop()
    }

    // MARK: - Channel Management

    public func subscribeChannel(_ channelId: String) {
        // TODO: Call Rust FFI
        // client?.subscribeChannel(channelId: channelId)
    }

    public func unsubscribeChannel(_ channelId: String) {
        // TODO: Call Rust FFI
        // client?.unsubscribeChannel(channelId: channelId)
    }

    // MARK: - Talk Control

    public func startTalk(on channelId: String) throws {
        guard isConnected else {
            throw IntercomError.notConnected
        }

        // TODO: Call Rust FFI
        // try client?.startTalk(channelId: channelId)

        currentChannel = channelId
        isTalking = true
    }

    public func stopTalk() {
        // TODO: Call Rust FFI
        // try? client?.stopTalk()

        isTalking = false
    }

    // MARK: - Error Handling

    public func clearError() {
        errorMessage = nil
    }
}

// MARK: - Supporting Types

public enum ConnectionState {
    case disconnected
    case connecting
    case connected
    case disconnecting
    case failed
}

public struct AudioDeviceInfo: Identifiable, Hashable {
    public let id = UUID()
    public let name: String
    public let isDante: Bool
    public let isDefault: Bool
}

public struct UserInfo: Identifiable {
    public let id: String
    public let displayName: String
    public var isTalking: Bool
}

public struct ChannelInfo: Identifiable {
    public let id: String
    public let name: String
    public let description: String?
}

public enum IntercomError: Error, LocalizedError {
    case notConnected
    case alreadyConnected
    case connectionFailed(String)
    case audioError(String)

    public var errorDescription: String? {
        switch self {
        case .notConnected:
            return "Not connected to a room"
        case .alreadyConnected:
            return "Already connected to a room"
        case .connectionFailed(let message):
            return "Connection failed: \(message)"
        case .audioError(let message):
            return "Audio error: \(message)"
        }
    }
}
