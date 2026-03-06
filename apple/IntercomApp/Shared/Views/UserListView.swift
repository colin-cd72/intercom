import SwiftUI

/// View displaying the list of users in the room
public struct UserListView: View {
    @EnvironmentObject var intercomManager: IntercomManager

    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Users")
                    .font(.headline)
                    .foregroundColor(.primary)

                Spacer()

                Text("\(intercomManager.users.count)")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    #if os(macOS)
                    .background(Color(.controlBackgroundColor))
                    #else
                    .background(Color(.tertiarySystemBackground))
                    #endif
                    .clipShape(Capsule())
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            #if os(macOS)
            .background(Color(.controlBackgroundColor))
            #else
            .background(Color(.secondarySystemBackground))
            #endif

            Divider()

            // User list
            if intercomManager.users.isEmpty {
                emptyState
            } else {
                userList
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "person.2.slash")
                .font(.system(size: 40))
                .foregroundColor(.secondary)

            Text("No Users")
                .font(.headline)
                .foregroundColor(.secondary)

            Text("Connect to a room to see other users")
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var userList: some View {
        List {
            ForEach(intercomManager.users) { user in
                UserRow(user: user)
            }
        }
        .listStyle(.plain)
    }
}

struct UserRow: View {
    let user: UserInfo

    var body: some View {
        HStack(spacing: 12) {
            // Avatar
            ZStack {
                Circle()
                    .fill(avatarColor)
                    .frame(width: 36, height: 36)

                Text(initials)
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundColor(.white)

                // Talking indicator
                if user.isTalking {
                    Circle()
                        .stroke(Color.green, lineWidth: 2)
                        .frame(width: 40, height: 40)
                        .scaleEffect(user.isTalking ? 1.1 : 1.0)
                        .animation(
                            Animation.easeInOut(duration: 0.5)
                                .repeatForever(autoreverses: true),
                            value: user.isTalking
                        )
                }
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(user.displayName)
                    .font(.body)
                    .foregroundColor(.primary)

                HStack(spacing: 4) {
                    if user.isTalking {
                        Image(systemName: "waveform")
                            .font(.caption2)
                            .foregroundColor(.green)

                        Text("Talking")
                            .font(.caption)
                            .foregroundColor(.green)
                    } else {
                        Text("Connected")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
            }

            Spacer()

            // Volume/mute controls
            #if os(macOS)
            Menu {
                Button("Mute") {}
                Button("Adjust Volume...") {}
                Divider()
                Button("View Profile") {}
            } label: {
                Image(systemName: "ellipsis")
                    .foregroundColor(.secondary)
            }
            .menuStyle(.borderlessButton)
            .frame(width: 20)
            #endif
        }
        .padding(.vertical, 4)
    }

    private var initials: String {
        let components = user.displayName.components(separatedBy: " ")
        let initials = components.prefix(2).compactMap { $0.first }
        return String(initials).uppercased()
    }

    private var avatarColor: Color {
        // Generate consistent color based on user ID
        let hash = user.id.hashValue
        let hue = Double(abs(hash) % 360) / 360.0
        return Color(hue: hue, saturation: 0.6, brightness: 0.7)
    }
}

#if DEBUG
struct UserListView_Previews: PreviewProvider {
    static var previews: some View {
        UserListView()
            .environmentObject(IntercomManager.shared)
            .frame(width: 250, height: 400)
    }
}
#endif
