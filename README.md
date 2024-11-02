# Heartbeat

A simple heartbeat server to monitor the health of your servers.

## Usage

Clone this repository and run `cargo build --release` to build the binary. The binary will be located in `target/release/heartbeat`.

Copy the config.example.toml to config.toml and adjust the settings to your needs.
This config file is required to run the server and needs to be in the PWD where you run the binary (this might not be the same as the binary location).

You NEED an SMTP server to send emails.

Please generate v4 UUIDs for each server. They need to be unique and valid. Generate them online.

### Send heartbeats

To send a heartbeat, you need to send a UDP packet to port 28915 of the server every 10 seconds. The packet simply needs to contain the UUID of the server.
