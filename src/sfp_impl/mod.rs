extern crate libc;
use self::libc::{c_int, size_t};
use std::sync::Arc;

#[repr(C)]
struct _sfpContext(*mut libc::c_void);

impl Clone for _sfpContext {
    fn clone(&self) -> _sfpContext {
        let new_ptr = *self;
        new_ptr
    }
}
impl Copy for _sfpContext {}

unsafe impl Send for _sfpContext {}
unsafe impl Sync for _sfpContext {}

//#[link(name = "sfp", kind = "static")]
extern {
    fn sfpDeliverOctet(ctx: _sfpContext, 
                       octet: u8, 
                       buf: *mut u8,
                       len: size_t,
                       outlen: *mut size_t) -> c_int;

    fn sfpWritePacket(ctx: _sfpContext,
                      buf: *const u8,
                      len: size_t,
                      outlen: *mut size_t) -> c_int;

    fn sfpConnect(ctx: _sfpContext);
    fn sfpIsConnected(ctx: _sfpContext) -> c_int;

    fn sfpGetSizeof() -> size_t;
    fn sfpInit(ctx: _sfpContext);

    fn sfpSetWriteCallback(ctx: _sfpContext,
                           writeType: u32,
                           cbfun: extern fn( octets: *mut u8, 
                                             len: size_t, 
                                             outlen: *mut size_t, 
                                             userdata: *mut Context ) -> c_int,
                           userdata: *mut Context);
    fn sfpNew() -> _sfpContext;
}

const BUFSIZE : size_t = 512;

#[repr(C)]
pub struct Context {
    ctx: _sfpContext,
    buf: Vec<u8>,
    deliver_cb: Option<Box<FnMut(&Vec<u8>)>>,
    write_cb: Option<Box<FnMut(&[u8]) -> usize>>,
}

extern "C" fn _write_callback(octets: *mut u8,
                              len: size_t,
                              outlen: *mut size_t,
                              target: *mut Context) -> c_int {
    unsafe {
        match (*target).write_cb {
            Some(ref mut func) => {
                let data = Vec::from_raw_parts(octets, len, len);
                let sent_len = (*func)( data.as_slice() );
                *outlen = sent_len;
            }
            _ => {}
        }
    }
    len as c_int
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl Context{
    pub fn new() -> Box<Context> {
        unsafe {
            let ctx = Context {
                ctx: sfpNew(),
                buf: Vec::with_capacity( BUFSIZE ),
                deliver_cb : None,
                write_cb: None
            };
            let mut ctx_box = Box::new(ctx);
            sfpSetWriteCallback(ctx_box.ctx,
                                1, // this is SFP_WRITE_MULTIPLE from serial_framing_protocol.h
                                _write_callback,
                                &mut *ctx_box);

            return ctx_box;
        }
    }

    pub fn connect(&mut self) {
        unsafe {
            sfpConnect(self.ctx);
        }
    }

    pub fn is_connected(&mut self) -> bool {
        unsafe {
            sfpIsConnected(self.ctx) != 0
        }
    }

    // When bytes are received from the underlying transport, give them to this
    // function.
    pub fn deliver(&mut self, octet: u8) -> Option<Vec<u8>> {
        unsafe {
            let mut outsize : size_t = 0;
            if sfpDeliverOctet(self.ctx, 
                               octet, 
                               self.buf.as_mut_ptr(), 
                               BUFSIZE, 
                               &mut outsize) > 0 {
                // If there is a write callback, call it
                match self.deliver_cb {
                    Some(ref mut func) => {
                        func(&self.buf.clone());
                    },
                    _ => {}
                }
                println!("deliver returning data...");
                self.buf.set_len(outsize);
                Some(self.buf.clone())
            } else {
                None
            }
        }
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        unsafe {
            let len = data.len() as size_t;
            let mut outlen: size_t = 0;
            sfpWritePacket(self.ctx,
                           data.as_ptr(),
                           len,
                           &mut outlen);
            return outlen;
        }
    }

    pub fn set_write_callback<F>(&mut self, callback: F) 
        where F: FnMut(&[u8]) -> usize,
              F: 'static
    {
        self.write_cb = Some(Box::new(callback));
    }
}
