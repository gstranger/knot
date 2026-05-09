#!/bin/bash
# Download one chunk of the ABC dataset (STEP files).
# Each chunk contains ~10,000 STEP models, ~635MB compressed.
#
# Usage: ./scripts/fetch_abc_chunk.sh [chunk_number]
#   chunk_number: 0-99 (default: 0)

set -euo pipefail

CHUNK=${1:-0}
CHUNK_PAD=$(printf "%04d" "$CHUNK")
DEST_DIR="data/abc"
ARCHIVE="abc_${CHUNK_PAD}_step_v00.7z"
URL="https://archive.nyu.edu/rest/fb/data/abc/${ARCHIVE}"

mkdir -p "$DEST_DIR"

if [ -d "$DEST_DIR/$CHUNK_PAD" ] && [ "$(ls "$DEST_DIR/$CHUNK_PAD"/*.step 2>/dev/null | head -1)" ]; then
    echo "Chunk $CHUNK_PAD already extracted at $DEST_DIR/$CHUNK_PAD"
    echo "$(ls "$DEST_DIR/$CHUNK_PAD"/*.step | wc -l | tr -d ' ') STEP files"
    exit 0
fi

echo "Downloading ABC chunk $CHUNK_PAD (~635MB)..."
echo "URL: $URL"

if [ ! -f "$DEST_DIR/$ARCHIVE" ]; then
    curl -L -o "$DEST_DIR/$ARCHIVE" "$URL"
else
    echo "Archive already downloaded"
fi

echo "Extracting..."
mkdir -p "$DEST_DIR/$CHUNK_PAD"
7z x -o"$DEST_DIR/$CHUNK_PAD" "$DEST_DIR/$ARCHIVE" -y > /dev/null

COUNT=$(find "$DEST_DIR/$CHUNK_PAD" -name "*.step" -o -name "*.stp" | wc -l | tr -d ' ')
echo "Extracted $COUNT STEP files to $DEST_DIR/$CHUNK_PAD"

# Clean up archive to save disk space
# rm "$DEST_DIR/$ARCHIVE"
echo "Done. Archive kept at $DEST_DIR/$ARCHIVE"
