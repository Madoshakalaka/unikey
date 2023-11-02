#!/usr/bin/env bash
#
/usr/bin/cargo build -p server --release
sudo ln -fs $(realpath target/release/server) /usr/local/bin/unikey-server
pkill unikey-server
nohup /usr/local/bin/unikey-server &

