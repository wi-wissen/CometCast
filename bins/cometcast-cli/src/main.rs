// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast command line tool. For now it only prints version and help.

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    // Skip the program name, keep the arguments.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("cometcast-cli {version}");
    } else {
        println!("cometcast-cli {version}");
        println!("Usage: cometcast-cli [--version]");
        println!("More commands (send, receive) will come later.");
    }
}
