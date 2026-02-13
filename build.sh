#! /bin/bash

echo "building tinyTetris in $(pwd)"

cargo build --release --target target.json -Zjson-target-spec

echo "tinyTetris built"
