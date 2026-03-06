# Build Instructions

## Prerequisites

### All Platforms
- Rust 1.75+ with `rustup`
- Opus codec development libraries

### macOS/iOS
- Xcode 15+
- CocoaPods or Swift Package Manager

### Windows
- Visual Studio 2022 with C++ workload
- Qt 6.5+ (with Multimedia module)
- CMake 3.16+

## Building the Rust Core

```bash
cd rust

# Build for development
cargo build

# Build for release
cargo build --release

# Run tests
cargo test
```

### Cross-compilation targets

```bash
# Add targets
rustup target add aarch64-apple-darwin     # macOS Apple Silicon
rustup target add x86_64-apple-darwin      # macOS Intel
rustup target add aarch64-apple-ios        # iOS
rustup target add x86_64-pc-windows-msvc   # Windows

# Build for specific target
cargo build --release --target aarch64-apple-darwin
```

## Building the macOS App

1. Generate Swift bindings:
```bash
cd rust/crates/intercom-ffi
cargo build --release
# UniFFI generates Swift files automatically
```

2. Build XCFramework:
```bash
# Create universal binary
lipo -create \
  target/aarch64-apple-darwin/release/libintercom_ffi.a \
  target/x86_64-apple-darwin/release/libintercom_ffi.a \
  -output libintercom_ffi.a

# Create XCFramework
xcodebuild -create-xcframework \
  -library libintercom_ffi.a \
  -headers include/ \
  -output IntercomCore.xcframework
```

3. Open in Xcode:
```bash
open apple/IntercomApp/IntercomApp.xcodeproj
```

4. Build and run from Xcode.

## Building the iOS App

1. Follow macOS steps 1-2 for iOS target.

2. In Xcode, select iOS target and device/simulator.

3. Build and run.

## Building the Windows App

1. Build Rust library:
```bash
cd rust
cargo build --release --target x86_64-pc-windows-msvc
```

2. Copy DLL and headers:
```bash
mkdir -p windows/IntercomCore/lib
mkdir -p windows/IntercomCore/bin
mkdir -p windows/IntercomCore/include

cp target/x86_64-pc-windows-msvc/release/intercom_ffi.dll windows/IntercomCore/bin/
cp target/x86_64-pc-windows-msvc/release/intercom_ffi.dll.lib windows/IntercomCore/lib/intercom_ffi.lib
# Copy generated headers
```

3. Build Qt app:
```bash
cd windows/IntercomApp
mkdir build && cd build
cmake .. -DCMAKE_PREFIX_PATH=/path/to/Qt/6.5.0/msvc2019_64
cmake --build . --config Release
```

## Firebase Setup

1. Create a Firebase project at https://console.firebase.google.com

2. Enable Realtime Database

3. Set database rules from `firebase/database.rules.json`

4. Copy your database URL (e.g., `https://your-project.firebaseio.com`)

5. Update the configuration in your app

## DANTE Setup

DANTE devices appear as virtual soundcards when the DANTE Virtual Soundcard (DVS) software is installed.

1. Install DANTE Virtual Soundcard from Audinate
2. Configure DVS with appropriate channel count
3. The intercom app will auto-detect "Dante Virtual Soundcard" in device lists
4. Enable "Prefer DANTE" in settings to auto-select DANTE devices

## Troubleshooting

### Audio Issues
- Ensure Opus codec libraries are installed
- Check device permissions (especially on macOS/iOS)
- Verify sample rate compatibility (48kHz recommended)

### Connection Issues
- Check Firebase database rules
- Verify network connectivity
- Check STUN/TURN server availability

### Build Errors
- Update Rust: `rustup update`
- Clean build: `cargo clean`
- Check dependencies: `cargo update`
