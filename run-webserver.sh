#!/bin/sh
cd ~/www/rust
ROCKET_ADDRESS=0.0.0.0 ROCKET_LOG_LEVEL=normal ./target/release/shorturls
