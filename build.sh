#!/bin/sh
cd ~/www/rust
time jsub -N build -mem 2G -sync y cargo +nightly build --release
