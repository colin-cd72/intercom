import SwiftUI

/// Large push-to-talk button view
public struct TalkButtonView: View {
    @EnvironmentObject var intercomManager: IntercomManager

    let channelId: String
    @State private var isPressed = false

    public init(channelId: String) {
        self.channelId = channelId
    }

    public var body: some View {
        VStack(spacing: 16) {
            // Status text
            Text(statusText)
                .font(.caption)
                .foregroundColor(.secondary)

            // Main talk button
            talkButton

            // Audio level indicator
            if intercomManager.isTalking {
                audioLevelIndicator
            }

            // Instructions
            Text(instructionText)
                .font(.caption2)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
    }

    private var statusText: String {
        if !intercomManager.isConnected {
            return "Not Connected"
        } else if intercomManager.isTalking {
            return "Transmitting..."
        } else {
            return "Ready"
        }
    }

    private var instructionText: String {
        #if os(iOS)
        return "Hold to talk"
        #else
        return "Hold to talk • Space bar"
        #endif
    }

    private var talkButton: some View {
        Circle()
            .fill(buttonColor)
            .frame(width: 120, height: 120)
            .overlay(
                VStack(spacing: 4) {
                    Image(systemName: intercomManager.isTalking ? "waveform" : "mic.fill")
                        .font(.system(size: 32))
                        .foregroundColor(.white)

                    Text(intercomManager.isTalking ? "TALKING" : "TALK")
                        .font(.caption)
                        .fontWeight(.bold)
                        .foregroundColor(.white)
                }
            )
            .shadow(color: shadowColor, radius: intercomManager.isTalking ? 20 : 8, x: 0, y: 4)
            .scaleEffect(isPressed ? 0.95 : 1.0)
            .animation(.easeInOut(duration: 0.1), value: isPressed)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        if !isPressed {
                            isPressed = true
                            startTalking()
                        }
                    }
                    .onEnded { _ in
                        isPressed = false
                        stopTalking()
                    }
            )
            .disabled(!intercomManager.isConnected)
            .opacity(intercomManager.isConnected ? 1.0 : 0.5)
    }

    private var buttonColor: Color {
        if !intercomManager.isConnected {
            return Color.gray
        } else if intercomManager.isTalking {
            return Color.red
        } else {
            return Color.green
        }
    }

    private var shadowColor: Color {
        if intercomManager.isTalking {
            return Color.red.opacity(0.5)
        } else {
            return Color.black.opacity(0.2)
        }
    }

    private var audioLevelIndicator: some View {
        VStack(spacing: 4) {
            HStack(spacing: 2) {
                ForEach(0..<20, id: \.self) { index in
                    Rectangle()
                        .fill(levelBarColor(for: index))
                        .frame(width: 8, height: levelBarHeight(for: index))
                }
            }
            .frame(height: 30)

            Text("Input Level")
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }

    private func levelBarColor(for index: Int) -> Color {
        let normalizedLevel = Int(intercomManager.inputLevel * 20)
        if index < normalizedLevel {
            if index >= 16 {
                return .red
            } else if index >= 12 {
                return .yellow
            } else {
                return .green
            }
        }
        return Color.gray.opacity(0.3)
    }

    private func levelBarHeight(for index: Int) -> CGFloat {
        let normalizedLevel = Int(intercomManager.inputLevel * 20)
        if index < normalizedLevel {
            return 30
        }
        return 10
    }

    private func startTalking() {
        guard intercomManager.isConnected else { return }

        do {
            try intercomManager.startTalk(on: channelId)

            #if os(iOS)
            // Haptic feedback on iOS
            let generator = UIImpactFeedbackGenerator(style: .medium)
            generator.impactOccurred()
            #endif
        } catch {
            // Handle error
        }
    }

    private func stopTalking() {
        intercomManager.stopTalk()
    }
}

#if DEBUG
struct TalkButtonView_Previews: PreviewProvider {
    static var previews: some View {
        TalkButtonView(channelId: "channel-1")
            .environmentObject(IntercomManager())
            .frame(width: 200, height: 250)
    }
}
#endif
