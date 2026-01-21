#!/bin/bash
# Generate icons for Tauri app
# Requires: ImageMagick

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ICON_SRC="$SCRIPT_DIR/../assets/icon.png"
ICON_DST="$SCRIPT_DIR/../src-tauri/icons"

# Create icon source if it doesn't exist
if [ ! -f "$ICON_SRC" ]; then
    echo "Creating placeholder icon..."
    mkdir -p "$(dirname "$ICON_SRC")"

    # Create a simple 512x512 icon with ImageMagick
    convert -size 512x512 xc:#6366f1 \
        -fill white -pointsize 200 -gravity center \
        -annotate +0+0 "SS" \
        "$ICON_SRC" 2>/dev/null || {
        echo "Warning: ImageMagick not found. Please install ImageMagick or provide $ICON_SRC"
        echo "You can also use any PNG editor to create a 512x512 icon"
    }
fi

# Generate Tauri icons if icon source exists
if [ -f "$ICON_SRC" ]; then
    echo "Generating Tauri icons..."

    mkdir -p "$ICON_DST"

    # Generate various icon sizes
    for size in 32 128 256 512; do
        convert "$ICON_SRC" -resize ${size}x${size} "$ICON_DST/${size}x${size}.png" 2>/dev/null
    done

    # Generate @2x icons
    convert "$ICON_SRC" -resize 256x256 "$ICON_DST/128x128@2x.png" 2>/dev/null
    convert "$ICON_SRC" -resize 1024x1024 "$ICON_DST/512x512@2x.png" 2>/dev/null

    echo "Icons generated successfully!"
else
    echo "Skipping icon generation. Run this script after providing $ICON_SRC"
fi
