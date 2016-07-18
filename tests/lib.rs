extern crate sfp;
extern crate mioco;

use std::net::{SocketAddr, TcpStream};
use std::io::{self, Read, Write};
use std::str::FromStr;
use mioco::tcp::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;

const DEFAULT_LISTEN_ADDR : &'static str = "127.0.0.1:0";

fn listend_addr() -> SocketAddr {
        FromStr::from_str(DEFAULT_LISTEN_ADDR).unwrap()
}

#[test]
fn hello() {

    mioco::start( || -> io::Result<()> {
        let testdata = Box::new("This is a test string.");
        let testdata2 = testdata.clone();

        let addr = listend_addr();
        let listener = TcpListener::bind(&addr).unwrap();
        let local_addr = listener.local_addr().unwrap();

        // Start the server
        mioco::spawn(move || -> io::Result<()> {
            let mut ctx1 = sfp::Context::new();
            let mut conn = try!(listener.accept());
            let mut conn_clone = try!(conn.try_clone());
            ctx1.set_write_callback( move |data : &[u8]| -> usize {
                let _ = conn_clone.write_all(&data);
                data.len()
            });
            let mut buf = [0u8; 1024];
            'mainloop: loop {
                let size = try!(conn.read(&mut buf));
                for i in 0..size{
                    let result = ctx1.deliver(buf[i]);
                    match result {
                        Some(str) => { assert!(str == testdata.to_string().into_bytes()); break 'mainloop; }
                        _ => {}
                    }
                }
            }
            Ok(())
        });

        // Start the client
        let mut stream = TcpStream::connect(local_addr).unwrap();
        let mut ctx2 = sfp::Context::new();
        let mut stream_clone = try!(stream.try_clone());
        ctx2.set_write_callback( move | data : &[u8]| -> usize {
            stream_clone.write(data).unwrap()
        });
        let mut ctx2_box = Arc::new(Mutex::new(ctx2));
        let mut stream_clone = try!(stream.try_clone());
        let mut ctx2_clone = ctx2_box.clone();
        mioco::spawn( move || -> io::Result<()> {
            let mut buf = [0u8; 1024];
            loop {
                let size = try!(stream_clone.read(&mut buf));
                for i in 0..size {
                    ctx2_clone.lock().unwrap().deliver(buf[i]);
                }
            }
            Ok(())
        });
        std::thread::sleep(std::time::Duration::new(1,0));
        ctx2_box.lock().unwrap().connect();
        while !ctx2_box.lock().unwrap().is_connected() {
            std::thread::sleep(std::time::Duration::new(1,0));
        }
        ctx2_box.lock().unwrap().write(testdata2.as_bytes());


        Ok(())
    }).unwrap().unwrap();
}

