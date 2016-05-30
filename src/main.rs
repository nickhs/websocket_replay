#![feature(plugin)]
#![plugin(docopt_macros)]
extern crate docopt;
extern crate rustc_serialize;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate ws;

use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

docopt!(Args derive Debug, "
websocket_replay

Playback files as websocket messages when a client connects.

Usage:
    websocket_replay [-c COUNT | -p PERC] [-t TIME] [-n | -0] <file>

Options:
    -n         records are newline seperated [default]
    -0         records are null byte seperated
    -p PERC    percentage of records to play upfront [default: 0.8]
    -c COUNT   count of records to play upfront
    -t TIME    time to wait between messages in seconds [default: 1]
");

#[derive(Debug)]
enum UpfrontPlayback {
    Perc(f32),
    Count(usize),
}

#[derive(Debug)]
struct SessionArgs {
    delim: u8,
    path: String,
    timeout: Duration,
    playback: UpfrontPlayback,
}

#[derive(Debug)]
struct Session<'a> {
    fh: BufReader<File>,
    sender: ws::Sender,
    active: bool,
    sess_args: &'a SessionArgs,
}

impl<'a> Session<'a> {
    fn new(sess_args: &'a SessionArgs,
           sender: ws::Sender) -> Session<'a> {
        let path = Path::new(&sess_args.path);
        let fh = File::open(path).expect("Could not open file");
        let fh = BufReader::new(fh);

        Session {
            fh: fh,
            sender: sender,
            active: true,
            sess_args: sess_args,
        }
    }

    fn replay_lines(&mut self, count: usize) {
        let mut buf = Vec::new();
        for _ in 0..count {
            let res = self.fh.read_until(self.sess_args.delim, &mut buf);
            let bytes_read = res.expect("cannot read file");
            if bytes_read == 0 {
                self.active = false;
            }

            let res = self.sender.send(&buf[..]);
            res.expect("cannot send data");
        }
    }

    fn replay_upfront(&mut self) {
        match self.sess_args.playback {
            UpfrontPlayback::Perc(perc) => self.replay_perc(perc),
            UpfrontPlayback::Count(count) => self.replay_lines(count),
        };
    }

    fn replay_perc(&mut self, perc: f32) {
        let mut buf = Vec::new();
        let metadata = self.fh.get_ref().metadata()
            .expect("could not read metadata about file");

        let file_size = metadata.len() as f32;
        let mut total_bytes_read: usize = 0;
        while (file_size * perc) > total_bytes_read as f32 {
            let res = self.fh.read_until(self.sess_args.delim, &mut buf);
            let bytes_read = res.expect("cannot read file");
            let res = self.sender.send(&buf[..]);
            res.expect("cannot send data");

            total_bytes_read += bytes_read;
        }
    }

    fn is_done(&self) -> bool {
        return !self.active;
    }

    /**
     * Get the timeout value (in ms)
     */
    fn get_timeout(&self) -> u64 {
        let timeout = self.sess_args.timeout.as_secs();
        timeout * 1000
    }
}

impl<'a> ws::Handler for Session<'a> {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        info!("Got connection from {:?} - token {:?}",
              shake.remote_addr(), self.sender.token());
        debug!("handshake is {:?}", shake);
        self.replay_upfront();
        self.sender.timeout(self.get_timeout(), self.sender.token())
    }

    fn on_timeout(&mut self, event: ws::util::Token) -> ws::Result<()> {
        assert_eq!(event, self.sender.token());
        self.replay_lines(1);

        if !self.is_done() {
            self.sender.timeout(self.get_timeout(), self.sender.token())
        } else {
            Ok(())
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let args: Args = Args::docopt().decode().unwrap_or_else(|e| e.exit());
    info!("Starting with args {:?}", args);

    let delim;
    if args.flag_n {
        delim = b'\n';
    } else if args.flag_0 {
        delim = b'\0';
    } else {
        delim = b'\n'; // default
    }

    let timeout = Duration::from_secs(
        args.flag_t.parse().expect("coult not parse -t"));

    let playback;
    if args.flag_c != "" {
        playback = UpfrontPlayback::Count(args.flag_c.parse().expect("could not parse -c"));
    } else if args.flag_p != "" {
        playback = UpfrontPlayback::Perc(args.flag_p.parse().expect("could not parse -p"));
    } else {
        panic!("Neither -p nor -c set!");
    }

    let sess_args = SessionArgs {
        path: args.arg_file,
        delim: delim,
        timeout: timeout,
        playback: playback,
    };

    ws::listen("127.0.0.1:3333", |out| {
        Session::new(&sess_args, out)
    }).expect("can initialize a websocket server");
}
