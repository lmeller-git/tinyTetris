#! /bin/bash

echo "building tinyTetris in $(pwd)"

cargo build --release

echo "tinyTetris built"
