// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast server program. For now it only prints its version and configuration.

use cc_server::{ServerConfig, VERSION};

fn main() {
    let config = ServerConfig::default();
    println!("cometcast-server {VERSION}");
    println!("config: {config:?}");
    println!("(no networking yet)");
}
