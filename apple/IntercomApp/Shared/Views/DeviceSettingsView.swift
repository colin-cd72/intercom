import SwiftUI

/// View for selecting audio devices
public struct DeviceSettingsView: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @Environment(\.dismiss) private var dismiss

    public init() {}

    public var body: some View {
        #if os(macOS)
        macOSContent
        #else
        iOSContent
        #endif
    }

    #if os(macOS)
    private var macOSContent: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Audio Devices")
                    .font(.headline)
                Spacer()
                Button("Done") {
                    dismiss()
                }
                .keyboardShortcut(.escape)
            }
            .padding()
            .background(Color(.windowBackgroundColor))

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // DANTE preference
                    dantePreference

                    Divider()

                    // Input devices
                    inputSection

                    Divider()

                    // Output devices
                    outputSection
                }
                .padding()
            }
        }
        .frame(width: 450, height: 500)
    }
    #endif

    #if os(iOS)
    private var iOSContent: some View {
        NavigationView {
            Form {
                // DANTE preference
                Section {
                    dantePreference
                }

                // Input devices
                Section("Input Device") {
                    inputDeviceList
                }

                // Output devices
                Section("Output Device") {
                    outputDeviceList
                }
            }
            .navigationTitle("Audio Devices")
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
    #endif

    private var dantePreference: some View {
        Toggle(isOn: $intercomManager.preferDante) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Prefer DANTE Devices")
                    .font(.body)
                Text("Automatically select DANTE Virtual Soundcard when available")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .onChange(of: intercomManager.preferDante) { _ in
            intercomManager.refreshDevices()
        }
    }

    private var inputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Input Device", systemImage: "mic.fill")
                .font(.headline)

            inputDeviceList
        }
    }

    private var inputDeviceList: some View {
        VStack(spacing: 4) {
            ForEach(intercomManager.inputDevices) { device in
                DeviceRow(
                    device: device,
                    isSelected: intercomManager.selectedInputDevice == device.name,
                    onSelect: {
                        intercomManager.selectInputDevice(device.name)
                    }
                )
            }
        }
    }

    private var outputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Output Device", systemImage: "speaker.wave.2.fill")
                .font(.headline)

            outputDeviceList
        }
    }

    private var outputDeviceList: some View {
        VStack(spacing: 4) {
            ForEach(intercomManager.outputDevices) { device in
                DeviceRow(
                    device: device,
                    isSelected: intercomManager.selectedOutputDevice == device.name,
                    onSelect: {
                        intercomManager.selectOutputDevice(device.name)
                    }
                )
            }
        }
    }
}

struct DeviceRow: View {
    let device: AudioDeviceInfo
    let isSelected: Bool
    let onSelect: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 12) {
                // Selection indicator
                Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                    .foregroundColor(isSelected ? .blue : .secondary)

                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(device.name)
                            .font(.body)
                            .foregroundColor(.primary)

                        if device.isDante {
                            Text("DANTE")
                                .font(.caption2)
                                .fontWeight(.semibold)
                                .foregroundColor(.white)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.purple)
                                .clipShape(Capsule())
                        }

                        if device.isDefault {
                            Text("Default")
                                .font(.caption2)
                                .foregroundColor(.secondary)
                        }
                    }
                }

                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(isSelected ? Color.blue.opacity(0.1) : Color.clear)
            .cornerRadius(8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

#if DEBUG
struct DeviceSettingsView_Previews: PreviewProvider {
    static var previews: some View {
        DeviceSettingsView()
            .environmentObject(IntercomManager.shared)
    }
}
#endif
