import SwiftUI

/// Main window view for macOS
struct MainWindow: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @State private var showingDeviceSettings = false
    @State private var showingServerSettings = false
    @State private var isServerMode = false
    @State private var selectedChannelId: String = "channel-0"

    var body: some View {
        NavigationSplitView {
            sidebar
                .frame(minWidth: 200, idealWidth: 250, maxWidth: 300)
        } content: {
            userList
                .frame(minWidth: 200, idealWidth: 250, maxWidth: 300)
        } detail: {
            mainContent
        }
        .toolbar {
            toolbarContent
        }
        .sheet(isPresented: $intercomManager.showConnectSheet) {
            ConnectSheet(isServerMode: $isServerMode)
                .environmentObject(intercomManager)
        }
        .sheet(isPresented: $intercomManager.showAudioSettings) {
            DeviceSettingsView()
                .environmentObject(intercomManager)
        }
        .alert("Error", isPresented: $intercomManager.showError) {
            Button("OK") { intercomManager.clearError() }
        } message: {
            Text(intercomManager.errorMessage ?? "Unknown error")
        }
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            // Mode indicator
            HStack {
                Image(systemName: isServerMode ? "server.rack" : "person.fill")
                    .foregroundColor(.secondary)
                Text(isServerMode ? "Server Mode" : "Client Mode")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Spacer()
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(Color(.controlBackgroundColor))

            Divider()

            // Channel list
            ChannelListView()

            Divider()

            // Connection status
            connectionStatus
        }
        .background(Color(.windowBackgroundColor))
    }

    private var userList: some View {
        UserListView()
    }

    private var mainContent: some View {
        VStack(spacing: 24) {
            Spacer()

            // Room info
            if let roomId = intercomManager.roomId {
                VStack(spacing: 4) {
                    Text("Room ID")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(roomId)
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                }
            }

            // Talk button
            TalkButtonView(channelId: selectedChannelId)

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.textBackgroundColor))
    }

    private var connectionStatus: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)

            Text(statusText)
                .font(.caption)
                .foregroundColor(.secondary)

            Spacer()

            if intercomManager.isConnected {
                Button("Disconnect") {
                    intercomManager.disconnect()
                }
                .buttonStyle(.borderless)
                .font(.caption)
            }
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(Color(.controlBackgroundColor))
    }

    private var statusColor: Color {
        switch intercomManager.connectionState {
        case .connected:
            return .green
        case .connecting:
            return .yellow
        case .failed:
            return .red
        case .disconnected, .closed, .new:
            return .gray
        }
    }

    private var statusText: String {
        switch intercomManager.connectionState {
        case .connected:
            return "Connected"
        case .connecting:
            return "Connecting..."
        case .failed:
            return "Connection Failed"
        case .disconnected, .closed, .new:
            return "Disconnected"
        }
    }

    @ToolbarContentBuilder
    private var toolbarContent: some ToolbarContent {
        ToolbarItemGroup(placement: .primaryAction) {
            Button(action: { showingConnectSheet = true }) {
                Label("Connect", systemImage: "antenna.radiowaves.left.and.right")
            }
            .disabled(intercomManager.isConnected)

            Button(action: { showingDeviceSettings = true }) {
                Label("Audio Settings", systemImage: "speaker.wave.2")
            }
        }

        ToolbarItem(placement: .status) {
            if intercomManager.isTalking {
                HStack(spacing: 4) {
                    Image(systemName: "waveform")
                        .foregroundColor(.green)
                    Text("Transmitting")
                        .font(.caption)
                        .foregroundColor(.green)
                }
            }
        }
    }
}

struct ConnectSheet: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @Environment(\.dismiss) private var dismiss
    @Binding var isServerMode: Bool

    @State private var displayName = "User"
    @State private var roomId = ""
    @State private var serverName = "Intercom Server"
    @State private var password = ""
    @State private var isConnecting = false

    var body: some View {
        VStack(spacing: 20) {
            // Mode picker
            Picker("Mode", selection: $isServerMode) {
                Text("Join Room").tag(false)
                Text("Host Server").tag(true)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)

            if isServerMode {
                serverForm
            } else {
                clientForm
            }

            // Buttons
            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button(isServerMode ? "Start Server" : "Connect") {
                    Task {
                        await connect()
                    }
                }
                .keyboardShortcut(.return)
                .disabled(isConnecting || !isValid)
            }
            .padding()
        }
        .padding()
        .frame(width: 400)
    }

    private var clientForm: some View {
        Form {
            TextField("Display Name", text: $displayName)
            TextField("Room ID", text: $roomId)
                .textContentType(.none)
        }
        .formStyle(.grouped)
    }

    private var serverForm: some View {
        Form {
            TextField("Server Name", text: $serverName)
            SecureField("Password (optional)", text: $password)
        }
        .formStyle(.grouped)
    }

    private var isValid: Bool {
        if isServerMode {
            return !serverName.isEmpty
        } else {
            return !displayName.isEmpty && !roomId.isEmpty
        }
    }

    private func connect() async {
        isConnecting = true
        defer { isConnecting = false }

        // Update display name in manager
        intercomManager.displayName = displayName

        if isServerMode {
            intercomManager.startServer(
                name: serverName,
                password: password.isEmpty ? nil : password
            )
        } else {
            intercomManager.connectAsClient(to: roomId)
        }
        dismiss()
    }
}
