extern crate sfp;
extern crate mioco;

use std::net::SocketAddr;
use std::io::{self, Read, Write};
use std::str::FromStr;
use mioco::tcp::TcpListener;

const DEFAULT_LISTEN_ADDR : &'static str = "127.0.0.1:0";

fn listend_addr() -> SocketAddr {
        FromStr::from_str(DEFAULT_LISTEN_ADDR).unwrap()
}

fn main() {
    let testdata = "This is a test string.";
    let mut ctx1 = sfp::Context::new();
    let mut ctx2 = sfp::Context::new();

    let addr = listend_addr();
    let listener = TcpListener::bind(&addr).unwrap();
    let port = listener.local_addr().unwrap().port();

    mioco::start( || -> io::Result<()> {

        // Start the server
        mioco::spawn(|| -> io::Result<()> {
            let mut conn = try!(listener.accept());
            ctx1.set_write_callback( |data : &[u8]| -> usize {
                let _ = conn.write_all(&mut data);
                data.len()
            });
            let mut buf = [0u8; 1024];
            'mainloop: loop {
                let size = try!(conn.read(&mut buf));
                for b in buf {
                    let result = ctx1.deliver(b);
                    match result {
                        Some(str) => { assert!(str == testdata); break 'mainloop; }
                        _ => {}
                    }
                }
            }
            Ok(());
        });
    }).unwrap().unwrap();
}

