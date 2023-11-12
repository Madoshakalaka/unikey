#!/usr/bin/env bash
c b -p server --release
cd server
RUST_LOG=server=debug ../target/release/server
