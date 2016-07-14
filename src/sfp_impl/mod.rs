extern crate libc;
use self::libc::{c_int, size_t};

//#[link(name = "sfp", kind = "static")]
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
                                             userdata: *mut Context ),
                           userdata: *mut Context);
}

const BUFSIZE : size_t = 512;

#[repr(C)]
pub struct Context {
    ctx: Vec<u8>,
    buf: Vec<u8>,
    deliver_cb: Option<Box<FnMut(&Vec<u8>)>>,
    write_cb: Option<Box<FnMut(&[u8]) -> usize>>,
}

extern "C" fn _write_callback(octets: *mut u8,
                              len: size_t,
                              outlen: *mut size_t,
                              target: *mut Context) {
    unsafe {
        match (*target).write_cb {
            Some(ref mut func) => {
                let data = Vec::from_raw_parts(octets, len, len);
                let sent_len = func( data.as_slice() );
                *outlen = sent_len;
            }
            _ => {}
        }
    }
}

impl Context{
    pub fn new() -> Context {
        unsafe {
            let ctx = Context {
                ctx: Vec::with_capacity( sfpGetSizeof() ),
                buf: Vec::with_capacity( BUFSIZE ),
                deliver_cb : None,
                write_cb: None
            };
            let mut ctx_box = Box::new(ctx);
            sfpSetWriteCallback(ctx_box.ctx.as_mut_ptr() as *mut libc::c_void,
                                1, // this is SFP_WRITE_MULTIPLE from serial_framing_protocol.h
                                _write_callback,
                                &mut *ctx_box);

            return *ctx_box;
        }
    }

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
                match self.deliver_cb {
                    Some(ref mut func) => {
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

    pub fn write(&mut self, data: &[u8]) -> usize {
        unsafe {
            let len = data.len() as size_t;
            let mut outlen: size_t = 0;
            sfpWritePacket(self.ctx.as_mut_ptr() as *mut libc::c_void,
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
