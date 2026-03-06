import SwiftUI

@main
struct IntercomApp: App {
    @StateObject private var intercomManager = IntercomManager.shared

    var body: some Scene {
        #if os(macOS)
        WindowGroup {
            MainWindow()
                .environmentObject(intercomManager)
                .frame(minWidth: 800, minHeight: 600)
        }
        .windowStyle(.titleBar)
        .commands {
            CommandGroup(replacing: .newItem) { }

            CommandMenu("Intercom") {
                Button("Connect...") {
                    intercomManager.showConnectSheet = true
                }
                .keyboardShortcut("k", modifiers: .command)

                Button("Disconnect") {
                    intercomManager.disconnect()
                }
                .keyboardShortcut("d", modifiers: .command)
                .disabled(!intercomManager.isConnected)

                Divider()

                Button("Audio Settings...") {
                    intercomManager.showAudioSettings = true
                }
                .keyboardShortcut(",", modifiers: .command)
            }
        }

        #if os(macOS)
        Settings {
            SettingsView()
                .environmentObject(intercomManager)
        }
        #endif

        #else
        WindowGroup {
            ContentView()
                .environmentObject(intercomManager)
        }
        #endif
    }
}

#if os(macOS)
struct SettingsView: View {
    @EnvironmentObject var intercomManager: IntercomManager

    var body: some View {
        TabView {
            DeviceSettingsView()
                .tabItem {
                    Label("Audio", systemImage: "speaker.wave.2")
                }

            GeneralSettingsView()
                .tabItem {
                    Label("General", systemImage: "gear")
                }
        }
        .frame(width: 450, height: 300)
        .environmentObject(intercomManager)
    }
}

struct GeneralSettingsView: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @AppStorage("pushToTalk") private var pushToTalk = true
    @AppStorage("firebaseUrl") private var firebaseUrl = ""

    var body: some View {
        Form {
            Section("Communication") {
                Toggle("Push to Talk", isOn: $pushToTalk)
                    .help("When enabled, hold the talk button to transmit")
            }

            Section("Server") {
                TextField("Firebase URL", text: $firebaseUrl)
                    .textFieldStyle(.roundedBorder)
                    .help("Your Firebase Realtime Database URL")
            }
        }
        .padding()
    }
}
#endif
