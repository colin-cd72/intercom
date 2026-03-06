import SwiftUI

/// Main content view for iOS
struct ContentView: View {
    @StateObject private var intercomManager = IntercomManager()
    @State private var showingConnectSheet = false
    @State private var showingSettings = false
    @State private var selectedTab: Tab = .talk

    enum Tab {
        case channels
        case talk
        case users
    }

    var body: some View {
        TabView(selection: $selectedTab) {
            channelsTab
                .tabItem {
                    Label("Channels", systemImage: "list.bullet")
                }
                .tag(Tab.channels)

            talkTab
                .tabItem {
                    Label("Talk", systemImage: "mic.fill")
                }
                .tag(Tab.talk)

            usersTab
                .tabItem {
                    Label("Users", systemImage: "person.2.fill")
                }
                .tag(Tab.users)
        }
        .sheet(isPresented: $showingConnectSheet) {
            ConnectView()
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView()
        }
        .environmentObject(intercomManager)
        .onAppear {
            if !intercomManager.isConnected {
                showingConnectSheet = true
            }
        }
    }

    private var channelsTab: some View {
        NavigationView {
            ChannelListView()
                .navigationTitle("Channels")
                .toolbar {
                    ToolbarItem(placement: .navigationBarTrailing) {
                        Button(action: { showingSettings = true }) {
                            Image(systemName: "gear")
                        }
                    }
                }
        }
    }

    private var talkTab: some View {
        NavigationView {
            VStack {
                if intercomManager.isConnected {
                    Spacer()

                    // Room info
                    if let roomId = intercomManager.currentRoomId {
                        VStack(spacing: 4) {
                            Text("Room")
                                .font(.caption)
                                .foregroundColor(.secondary)
                            Text(roomId)
                                .font(.system(.caption, design: .monospaced))
                        }
                        .padding()
                    }

                    // Talk button
                    TalkButtonView(channelId: "channel-0")

                    Spacer()

                    // Status
                    connectionStatusBar
                } else {
                    disconnectedView
                }
            }
            .navigationTitle("Talk")
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    connectionButton
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: { showingSettings = true }) {
                        Image(systemName: "gear")
                    }
                }
            }
        }
    }

    private var usersTab: some View {
        NavigationView {
            UserListView()
                .navigationTitle("Users")
                .toolbar {
                    ToolbarItem(placement: .navigationBarTrailing) {
                        Button(action: { showingSettings = true }) {
                            Image(systemName: "gear")
                        }
                    }
                }
        }
    }

    private var disconnectedView: some View {
        VStack(spacing: 20) {
            Image(systemName: "antenna.radiowaves.left.and.right.slash")
                .font(.system(size: 60))
                .foregroundColor(.secondary)

            Text("Not Connected")
                .font(.title2)
                .foregroundColor(.primary)

            Text("Connect to a room to start using the intercom")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)

            Button(action: { showingConnectSheet = true }) {
                Label("Connect", systemImage: "antenna.radiowaves.left.and.right")
                    .font(.headline)
                    .padding(.horizontal, 24)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }

    private var connectionButton: some View {
        Group {
            if intercomManager.isConnected {
                Button(action: {
                    Task {
                        await intercomManager.disconnect()
                    }
                }) {
                    Text("Disconnect")
                        .foregroundColor(.red)
                }
            } else {
                Button(action: { showingConnectSheet = true }) {
                    Text("Connect")
                }
            }
        }
    }

    private var connectionStatusBar: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(Color.green)
                .frame(width: 8, height: 8)

            Text("Connected")
                .font(.caption)
                .foregroundColor(.secondary)

            Spacer()

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
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(Color(.secondarySystemBackground))
    }
}

struct ConnectView: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName = ""
    @State private var roomId = ""
    @State private var isConnecting = false

    var body: some View {
        NavigationView {
            Form {
                Section("Your Information") {
                    TextField("Display Name", text: $displayName)
                        .textContentType(.name)
                        .autocapitalization(.words)
                }

                Section("Room") {
                    TextField("Room ID", text: $roomId)
                        .textContentType(.none)
                        .autocapitalization(.none)
                        .disableAutocorrection(true)
                }

                Section {
                    Button(action: connect) {
                        HStack {
                            Spacer()
                            if isConnecting {
                                ProgressView()
                                    .padding(.trailing, 8)
                            }
                            Text(isConnecting ? "Connecting..." : "Connect")
                            Spacer()
                        }
                    }
                    .disabled(isConnecting || displayName.isEmpty || roomId.isEmpty)
                }
            }
            .navigationTitle("Connect")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
    }

    private func connect() {
        isConnecting = true

        Task {
            do {
                try await intercomManager.connect(roomId: roomId, displayName: displayName)
                dismiss()
            } catch {
                isConnecting = false
            }
        }
    }
}

struct SettingsView: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationView {
            Form {
                Section("Audio") {
                    NavigationLink {
                        DeviceSettingsView()
                    } label: {
                        HStack {
                            Label("Audio Devices", systemImage: "speaker.wave.2")
                            Spacer()
                            if intercomManager.preferDante {
                                Text("DANTE")
                                    .font(.caption)
                                    .foregroundColor(.purple)
                            }
                        }
                    }
                }

                Section("About") {
                    HStack {
                        Text("Version")
                        Spacer()
                        Text("1.0.0")
                            .foregroundColor(.secondary)
                    }
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: - App Entry Point

@main
struct IntercomIOSApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

#if DEBUG
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
#endif
