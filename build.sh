#! /bin/bash

echo "building example-rs in $(pwd)"

cargo build --release

echo "example-rs built"
