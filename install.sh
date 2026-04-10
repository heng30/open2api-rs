#!/usr/bin/env bash

TARGET_BIN_DIR="$HOME/.local/bin"

./bundle.sh

rm -rf "$TARGET_BIN_DIR"/open2api-dist
cp -rf ./dist "$TARGET_BIN_DIR"/open2api-dist