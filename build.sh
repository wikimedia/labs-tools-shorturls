#!/bin/bash
cd ~/www/rust
time jsub -N build -mem 2G -sync y -cwd cargo +nightly build --release
