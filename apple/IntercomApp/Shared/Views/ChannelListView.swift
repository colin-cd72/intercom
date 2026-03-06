import SwiftUI

/// View displaying the list of available channels
public struct ChannelListView: View {
    @EnvironmentObject var intercomManager: IntercomManager
    @State private var selectedChannelId: String?

    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Channels")
                    .font(.headline)
                    .foregroundColor(.primary)

                Spacer()

                #if os(macOS)
                Button(action: {}) {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .help("Add Channel")
                #endif
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            #if os(macOS)
            .background(Color(.controlBackgroundColor))
            #else
            .background(Color(.secondarySystemBackground))
            #endif

            Divider()

            // Channel list
            if intercomManager.channels.isEmpty {
                emptyState
            } else {
                channelList
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "waveform.badge.plus")
                .font(.system(size: 40))
                .foregroundColor(.secondary)

            Text("No Channels")
                .font(.headline)
                .foregroundColor(.secondary)

            Text("Connect to a room to see available channels")
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var channelList: some View {
        List(selection: $selectedChannelId) {
            ForEach(intercomManager.channels) { channel in
                ChannelRow(
                    channel: channel,
                    isSelected: selectedChannelId == channel.id,
                    onTap: {
                        selectedChannelId = channel.id
                        intercomManager.subscribeChannel(channel.id)
                    }
                )
            }
        }
        .listStyle(.plain)
    }
}

struct ChannelRow: View {
    let channel: ChannelInfo
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                // Channel indicator
                Circle()
                    .fill(isSelected ? Color.green : Color.gray)
                    .frame(width: 8, height: 8)

                VStack(alignment: .leading, spacing: 2) {
                    Text(channel.name)
                        .font(.body)
                        .foregroundColor(.primary)

                    if let description = channel.description {
                        Text(description)
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                }

                Spacer()

                // Subscribed indicator
                if isSelected {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                }
            }
            .padding(.vertical, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

#if DEBUG
struct ChannelListView_Previews: PreviewProvider {
    static var previews: some View {
        ChannelListView()
            .environmentObject(IntercomManager.shared)
            .frame(width: 250, height: 400)
    }
}
#endif
