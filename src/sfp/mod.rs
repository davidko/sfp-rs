extern crate libc;
use self::libc::{c_int, size_t};

#[link(name = "sfp", kind = "static")]
extern {
    fn sfpDeliverOctet(ctx: *mut libc::c_void, 
                       octet: u8, 
                       buf: *mut u8,
                       len: size_t,
                       outlen: *mut size_t) -> c_int;

    fn sfpWritePacket(ctx: *mut libc::c_void,
                      buf: *const u8,
                      len: size_t,
                      outlen: *mut size_t) -> c_int;

    fn sfpConnect(ctx: *mut libc::c_void);

    fn sfpGetSizeof() -> size_t;
    fn sfpInit(ctx: *mut libc::c_void);

    fn sfpSetWriteCallback(ctx: *mut libc::c_void,
                           writeType: u32,
                           cbfun: extern fn( octets: *mut u8, 
                                             len: size_t, 
                                             outlen: *mut size_t, 
                                             userdata: *mut libc::c_void ),
                           userdata: *mut libc::c_void);
}

const BUFSIZE : size_t = 512;

pub struct Context {
    ctx: Vec<u8>,
    buf: Vec<u8>,
    write_cb: Option<fn(&Vec<u8>)>,
}

pub fn new() -> Context {
    unsafe {
        Context {
            ctx: Vec::with_capacity( sfpGetSizeof() ),
            buf: Vec::with_capacity( BUFSIZE ),
            write_cb: None
        }
    }
}

impl Context{
    // When bytes are received from the underlying transport, give them to this
    // function.
    pub fn deliver(&mut self, octet: u8) -> Option<Vec<u8>> {
        unsafe {
            let mut outsize : size_t = 0;
            if sfpDeliverOctet(self.ctx.as_mut_ptr() as *mut libc::c_void, 
                               octet, 
                               self.buf.as_mut_ptr(), 
                               BUFSIZE, 
                               &mut outsize) > 0 {
                // If there is a write callback, call it
                match self.write_cb {
                    Some(func) => {
                        func(&self.buf.clone());
                    },
                    _ => {}
                }
                Some(self.buf.clone())
            } else {
                None
            }
        }
    }

    pub fn write(&mut self, data: &Vec<u8>) -> usize {
        unsafe {
            let len = data.len() as size_t;
            let pdata = data.as_ptr();
            let mut outlen: size_t = 0;
            sfpWritePacket(self.ctx.as_mut_ptr() as *mut libc::c_void,
                           pdata,
                           len,
                           &mut outlen);
            return outlen;
        }
    }
}
