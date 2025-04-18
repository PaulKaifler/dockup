#!/bin/bash

set -e

# Update the repository
echo "🔄 Pulling latest changes from git..."
git pull || { echo "❌ Failed to pull latest changes."; exit 1; }

# Build and install the project
echo "🔨 Building the project..."
cargo build --release || { echo "❌ Build failed."; exit 1; }

# Move the binary to the project home directory
BINARY_NAME="dockup"
PROJECT_HOME=$(pwd)
BINARY_PATH="$PROJECT_HOME/target/release/$BINARY_NAME"

if [ -f "$BINARY_PATH" ]; then
    echo "📦 Moving binary to project home directory..."
    mv "$BINARY_PATH" "$PROJECT_HOME" || { echo "❌ Failed to move binary."; exit 1; }
    echo "✅ Binary moved to $PROJECT_HOME/$BINARY_NAME"
else
    echo "❌ Binary not found at $BINARY_PATH. Build might have failed."
    exit 1
fi

# Make it available system-wide
echo "📁 Installing system-wide to /usr/local/bin..."
sudo cp "$PROJECT_HOME/$BINARY_NAME" /usr/local/bin/ || { echo "❌ Failed to install system-wide."; exit 1; }
sudo chmod +x /usr/local/bin/$BINARY_NAME

echo "🎉 Done. You can now run '$BINARY_NAME' from anywhere."