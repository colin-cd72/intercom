#!/bin/bash
# Build script for IntercomCore.xcframework
# This creates a universal framework from the Rust library

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/rust"
OUTPUT_DIR="$SCRIPT_DIR/Frameworks"

echo "Building Rust library..."

cd "$RUST_DIR"

# Set up environment for opus/cmake
export CMAKE_POLICY_VERSION_MINIMUM=3.5
export PKG_CONFIG_PATH="/opt/homebrew/lib/pkgconfig:/opt/homebrew/Cellar/opus/1.6.1/lib/pkgconfig:$PKG_CONFIG_PATH"

# Build for macOS (Apple Silicon)
echo "Building for macOS (arm64)..."
cargo build --release --target aarch64-apple-darwin -p intercom-ffi

# Build for macOS (Intel) - optional, comment out if not needed
echo "Building for macOS (x86_64)..."
cargo build --release --target x86_64-apple-darwin -p intercom-ffi

# Build for iOS device
echo "Building for iOS (arm64)..."
cargo build --release --target aarch64-apple-ios -p intercom-ffi

# Build for iOS simulator (Apple Silicon)
echo "Building for iOS Simulator (arm64)..."
cargo build --release --target aarch64-apple-ios-sim -p intercom-ffi

# Generate Swift bindings
echo "Generating Swift bindings..."
cd "$RUST_DIR"

# Create output directory for bindings
BINDINGS_DIR="$SCRIPT_DIR/IntercomApp/Shared/Generated"
mkdir -p "$BINDINGS_DIR"

# Build dylib for bindgen (uses host target)
echo "Building dylib for binding generation..."
cargo build --release -p intercom-ffi

# Generate bindings using uniffi-bindgen from the compiled library
# With proc macros, we extract the interface from the dylib
cargo run --release -p intercom-ffi --features cli --bin uniffi-bindgen -- \
    generate --library "$RUST_DIR/target/release/libintercom_ffi.dylib" \
    --language swift \
    --out-dir "$BINDINGS_DIR"

echo "Swift bindings generated at $BINDINGS_DIR"

# Create framework directories
echo "Creating framework structure..."
mkdir -p "$OUTPUT_DIR"

# Create universal macOS library
echo "Creating universal macOS library..."
MACOS_LIB="$OUTPUT_DIR/macos/libintercom_ffi.a"
mkdir -p "$(dirname "$MACOS_LIB")"
lipo -create \
    "$RUST_DIR/target/aarch64-apple-darwin/release/libintercom_ffi.a" \
    "$RUST_DIR/target/x86_64-apple-darwin/release/libintercom_ffi.a" \
    -output "$MACOS_LIB" 2>/dev/null || \
    cp "$RUST_DIR/target/aarch64-apple-darwin/release/libintercom_ffi.a" "$MACOS_LIB"

# Copy iOS libraries
echo "Copying iOS libraries..."
IOS_LIB="$OUTPUT_DIR/ios/libintercom_ffi.a"
IOS_SIM_LIB="$OUTPUT_DIR/ios-simulator/libintercom_ffi.a"
mkdir -p "$(dirname "$IOS_LIB")" "$(dirname "$IOS_SIM_LIB")"
cp "$RUST_DIR/target/aarch64-apple-ios/release/libintercom_ffi.a" "$IOS_LIB"
cp "$RUST_DIR/target/aarch64-apple-ios-sim/release/libintercom_ffi.a" "$IOS_SIM_LIB"

# Create module map
MODULE_MAP="module IntercomCore {
    header \"intercomFFI.h\"
    export *
}
"

# Create xcframework
echo "Creating xcframework..."
rm -rf "$OUTPUT_DIR/IntercomCore.xcframework"

xcodebuild -create-xcframework \
    -library "$MACOS_LIB" \
    -library "$IOS_LIB" \
    -library "$IOS_SIM_LIB" \
    -output "$OUTPUT_DIR/IntercomCore.xcframework"

echo ""
echo "Build complete!"
echo "XCFramework: $OUTPUT_DIR/IntercomCore.xcframework"
echo "Swift bindings: $BINDINGS_DIR"
echo ""
echo "Next steps:"
echo "1. Open IntercomApp.xcodeproj in Xcode"
echo "2. Add IntercomCore.xcframework to the project"
echo "3. Add the generated Swift files to the project"
