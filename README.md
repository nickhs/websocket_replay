# `websocket_replay`

Playback JSON records from a file when a socket connect.
Created for use when building a website that relies purely on websocket communication.

## Example

    $ ./websocket_replay -c 100 test_resources/log_file.json

## Usage

    websocket_replay [-c COUNT | -p PERC] [-t TIME] [-n | -0] <file>

## Build

    $ cargo build
