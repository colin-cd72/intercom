import SwiftUI
import Combine

/// Main manager for the intercom system.
/// Handles connection state, audio, and event processing.
@MainActor
public class IntercomManager: ObservableObject {
    public static let shared = IntercomManager()

    // MARK: - Published State

    @Published public var connectionState: ConnectionState = .disconnected
    @Published public var isConnected: Bool = false
    @Published public var isTalking: Bool = false
    @Published public var roomId: String?
    @Published public var userId: String?

    @Published public var channels: [ChannelInfo] = []
    @Published public var users: [UserInfo] = []
    @Published public var subscribedChannels: Set<String> = []
    @Published public var currentTalkChannel: String?

    @Published public var inputDevices: [AudioDeviceInfo] = []
    @Published public var outputDevices: [AudioDeviceInfo] = []
    @Published public var selectedInputDevice: String?
    @Published public var selectedOutputDevice: String?

    @Published public var inputLevel: Float = 0
    @Published public var outputLevel: Float = 0

    @Published public var errorMessage: String?
    @Published public var showError: Bool = false
    @Published public var showConnectSheet: Bool = false
    @Published public var showAudioSettings: Bool = false

    // MARK: - Configuration

    @AppStorage("displayName") public var displayName: String = "User"
    @AppStorage("firebaseUrl") public var firebaseUrl: String = ""
    @AppStorage("pushToTalk") public var pushToTalk: Bool = true
    @AppStorage("preferDante") public var preferDante: Bool = true

    // MARK: - Private

    private var client: IntercomClient?
    private var server: IntercomServer?
    private var eventPollTimer: Timer?
    private var isServerMode: Bool = false

    // MARK: - Initialization

    private init() {
        refreshDevices()
    }

    // MARK: - Device Management

    public func refreshDevices() {
        inputDevices = listInputDevices()
        outputDevices = listOutputDevices()

        // Auto-select DANTE devices if preferred
        if preferDante {
            if let dante = inputDevices.first(where: { $0.isDante }) {
                selectedInputDevice = dante.name
            }
            if let dante = outputDevices.first(where: { $0.isDante }) {
                selectedOutputDevice = dante.name
            }
        }

        // Fall back to default devices
        if selectedInputDevice == nil {
            selectedInputDevice = inputDevices.first(where: { $0.isDefault })?.name
        }
        if selectedOutputDevice == nil {
            selectedOutputDevice = outputDevices.first(where: { $0.isDefault })?.name
        }
    }

    public func selectInputDevice(_ name: String) {
        selectedInputDevice = name
        try? client?.setInputDevice(deviceName: name)
    }

    public func selectOutputDevice(_ name: String) {
        selectedOutputDevice = name
        try? client?.setOutputDevice(deviceName: name)
    }

    // MARK: - Client Mode

    public func connectAsClient(to roomId: String) {
        guard !firebaseUrl.isEmpty else {
            showErrorMessage("Firebase URL not configured. Go to Settings to configure.")
            return
        }

        isServerMode = false
        connectionState = .connecting

        let config = ClientConfiguration(
            displayName: displayName,
            inputDevice: selectedInputDevice,
            outputDevice: selectedOutputDevice,
            firebaseUrl: firebaseUrl,
            pushToTalk: pushToTalk,
            preferDante: preferDante
        )

        do {
            client = try IntercomClient(config: config)
            try client?.initAudio()
            try client?.connect(roomId: roomId)
            try client?.startAudio()

            self.roomId = roomId
            self.userId = client?.getUserId()
            self.isConnected = true
            self.connectionState = .connected

            startEventPolling()
        } catch {
            connectionState = .failed
            showErrorMessage("Failed to connect: \(error.localizedDescription)")
        }
    }

    // MARK: - Server Mode

    public func startServer(name: String, password: String? = nil, defaultChannels: [String] = ["Channel 1", "Channel 2"]) {
        guard !firebaseUrl.isEmpty else {
            showErrorMessage("Firebase URL not configured. Go to Settings to configure.")
            return
        }

        isServerMode = true
        connectionState = .connecting

        let config = ServerConfiguration(
            name: name,
            password: password,
            firebaseUrl: firebaseUrl,
            maxUsers: 100,
            defaultChannels: defaultChannels,
            preferDante: preferDante
        )

        do {
            server = try IntercomServer(config: config)
            try server?.initAudio()
            try server?.start()

            self.roomId = server?.getRoomId()
            self.isConnected = true
            self.connectionState = .connected

            // Refresh channel list
            self.channels = server?.getChannels() ?? []

            startEventPolling()
        } catch {
            connectionState = .failed
            showErrorMessage("Failed to start server: \(error.localizedDescription)")
        }
    }

    public func stopServer() {
        stopEventPolling()
        try? server?.stop()
        server = nil
        isConnected = false
        connectionState = .disconnected
        roomId = nil
    }

    // MARK: - Disconnect

    public func disconnect() {
        stopEventPolling()

        if isServerMode {
            try? server?.stop()
            server = nil
        } else {
            try? client?.stopAudio()
            try? client?.disconnect()
            client = nil
        }

        isConnected = false
        isTalking = false
        connectionState = .disconnected
        roomId = nil
        userId = nil
        channels = []
        users = []
        subscribedChannels = []
        currentTalkChannel = nil
    }

    // MARK: - Channels

    public func subscribeChannel(_ channelId: String) {
        client?.subscribeChannel(channelId: channelId)
        subscribedChannels.insert(channelId)
    }

    public func unsubscribeChannel(_ channelId: String) {
        client?.unsubscribeChannel(channelId: channelId)
        subscribedChannels.remove(channelId)
    }

    public func createChannel(name: String) {
        guard isServerMode else { return }
        do {
            let channelId = try server?.createChannel(name: name) ?? ""
            channels.append(ChannelInfo(id: channelId, name: name, description: nil))
        } catch {
            showErrorMessage("Failed to create channel: \(error.localizedDescription)")
        }
    }

    public func deleteChannel(_ channelId: String) {
        guard isServerMode else { return }
        do {
            try server?.deleteChannel(channelId: channelId)
            channels.removeAll { $0.id == channelId }
        } catch {
            showErrorMessage("Failed to delete channel: \(error.localizedDescription)")
        }
    }

    // MARK: - Talking

    public func startTalk(on channelId: String) {
        guard !isTalking else { return }

        do {
            try client?.startTalk(channelId: channelId)
            isTalking = true
            currentTalkChannel = channelId
        } catch {
            showErrorMessage("Failed to start talking: \(error.localizedDescription)")
        }
    }

    public func stopTalk() {
        guard isTalking else { return }

        do {
            try client?.stopTalk()
            isTalking = false
            currentTalkChannel = nil
        } catch {
            showErrorMessage("Failed to stop talking: \(error.localizedDescription)")
        }
    }

    // MARK: - Event Handling

    private func startEventPolling() {
        eventPollTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.pollEvents()
            }
        }
    }

    private func stopEventPolling() {
        eventPollTimer?.invalidate()
        eventPollTimer = nil
    }

    private func pollEvents() {
        let events: [IntercomEvent]

        if isServerMode {
            events = server?.getPendingEvents() ?? []
        } else {
            events = client?.getPendingEvents() ?? []
        }

        for event in events {
            handleEvent(event)
        }
    }

    private func handleEvent(_ event: IntercomEvent) {
        switch event {
        case .connectionStateChanged(let state):
            connectionState = state
            isConnected = (state == .connected)

        case .connected(let roomId, let oderId):
            self.roomId = roomId
            self.userId = oderId
            isConnected = true
            connectionState = .connected

        case .disconnected(let reason):
            isConnected = false
            connectionState = .disconnected
            if !reason.isEmpty {
                showErrorMessage("Disconnected: \(reason)")
            }

        case .userJoined(let user):
            if !users.contains(where: { $0.id == user.id }) {
                users.append(user)
            }

        case .userLeft(let oderId):
            users.removeAll { $0.id == oderId }

        case .userTalkStart(let oderId, _):
            if let index = users.firstIndex(where: { $0.id == oderId }) {
                let user = users[index]
                users[index] = UserInfo(id: user.id, displayName: user.displayName, isTalking: true)
            }

        case .userTalkStop(let oderId):
            if let index = users.firstIndex(where: { $0.id == oderId }) {
                let user = users[index]
                users[index] = UserInfo(id: user.id, displayName: user.displayName, isTalking: false)
            }

        case .channelCreated(let channel):
            if !channels.contains(where: { $0.id == channel.id }) {
                channels.append(channel)
            }

        case .channelDeleted(let channelId):
            channels.removeAll { $0.id == channelId }

        case .talkStarted(let channelId):
            isTalking = true
            currentTalkChannel = channelId

        case .talkStopped:
            isTalking = false
            currentTalkChannel = nil

        case .audioLevel(let input, let output):
            inputLevel = input
            outputLevel = output

        case .error(let message):
            showErrorMessage(message)
        }
    }

    // MARK: - Error Handling

    private func showErrorMessage(_ message: String) {
        errorMessage = message
        showError = true
    }

    public func clearError() {
        errorMessage = nil
        showError = false
    }
}
