extern crate sfp;
extern crate mioco;

use std::net::SocketAddr;
use std::io::{self, Read, Write};
use std::str::FromStr;
use mioco::tcp::{TcpListener, TcpStream};
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
        println!("Starting the server...");
        mioco::spawn(move || -> io::Result<()> {
            println!("Server coro started.");
            let mut ctx1 = sfp::Context::new();
            println!("Server waiting for connection...");
            let mut conn = try!(listener.accept());
            println!("Server connection accepted.");
            let mut conn_clone = try!(conn.try_clone());
            ctx1.set_write_callback( move |data : &[u8]| -> usize {
                println!("Server writing {} bytes...", data.len());
                conn_clone.write(&data).unwrap();
                data.len()
            });
            let mut buf = [0u8; 1024];
            'mainloop: loop {
                println!("Starting server read loop...");
                let size = try!(conn.read(&mut buf));
                println!("server read {} bytes.", size);
                for i in 0..size{
                    let result = ctx1.deliver(buf[i]);
                    match result {
                        Some(str) => {
                            assert!(str == testdata.to_string().into_bytes()); break 'mainloop; 
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        });
        println!("Starting the server...done");

        // Start the client
        let mut ctx2 = sfp::Context::new();
        let mut ctx2_box = Arc::new(Mutex::new(ctx2));
        let mut ctx2_clone = ctx2_box.clone();
        mioco::spawn( move || -> io::Result<()> {
            let mut stream = TcpStream::connect(&local_addr).unwrap();
            println!("Client stream connected.");
            let mut stream_clone = try!(stream.try_clone());
            {
                // Set the write callback
                ctx2_clone.lock().unwrap().set_write_callback( move | data : &[u8]| -> usize {
                    println!("Client writing {} bytes...", data.len());
                    stream_clone.write(data).unwrap()
                    });
            }
            let mut stream = stream.try_clone().unwrap();
            mioco::spawn(move || -> io::Result<()>{
                println!("Spawn in spawn!");
                let mut buf = [0u8; 1024];
                loop {
                    // Start the reader loop
                    let size = try!(stream.read(&mut buf));
                    println!("client read {} bytes.", size);
                    for i in 0..size {
                        {
                            ctx2_clone.lock().unwrap().deliver(buf[i]);
                        }
                    }
                }
                Ok(())
            });

            mioco::sleep(std::time::Duration::new(1,0));
            {
                println!("Connecting...");
                ctx2_box.lock().unwrap().connect();
                println!("Connecting...done");
            }
            Ok(())
        });

        /*
        {
            loop {
                {
                    if ctx2_box.lock().unwrap().is_connected() {
                        break;
                    }
                }
                println!("Waiting...");
                mioco::sleep(std::time::Duration::new(1,0));
            }
        }
        {
            ctx2_box.lock().unwrap().write(testdata2.as_bytes());
        }


        mioco::sleep(std::time::Duration::new(5,0));
        */
        Ok(())
    }).unwrap().unwrap();
}

