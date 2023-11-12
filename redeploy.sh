#!/usr/bin/env bash
#
# /usr/bin/cargo build --release --features dlib-face-recognition/build-native
# build-native doesn't seem to have a impact on the detection time
/usr/bin/cargo build --release 
sudo ln -fs $(realpath target/release/server) /usr/local/bin/unikey-server
sudo ln -fs $(realpath target/release/client) /usr/local/bin/unikey-client
pkill unikey-server
cd server
nohup /usr/local/bin/unikey-server &

