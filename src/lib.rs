
// #[macro_use] extern crate log;

use std::collections::VecDeque;
use std::mem;
use std::fmt;
use std::fmt::Display;

const SFP_ESC: u8 = 0x7d;
const SFP_FLAG: u8 = 0x7e;
const SFP_ESC_FLIP_BIT: u8 = 1<<5;
const SFP_CRC_GOOD: u16 = 0xf0b8;
const SFP_CRC_INIT: u16 = 0xffff;
const SFP_SEQ_INIT: u8 = 0;

type SeqNum = u8;
type Crc = u16;

// header format: ccss ssss where "c" are frame-type bits and "s" are sequence number bits
const FRAMETYPE_USR:u8 = 0;
const FRAMETYPE_RTX:u8 = 1;
const FRAMETYPE_NAK:u8 = 2;
const FRAMETYPE_SYN:u8 = 3;

const SEQ_SYN0:u8 = 0;
const SEQ_SYN1:u8 = 1;
const SEQ_SYN2:u8 = 2;
const SEQ_SYN_DIS:u8 = 3;

const HISTORY_SIZE:usize = 32;

#[derive(Clone, PartialEq)]
pub enum SfpPacket {
    Usr{seq: u8, buf: Vec<u8>},
    Rtx{seq: u8, buf: Vec<u8>},
    Nak{seq: u8},
    Syn{seq: u8},
}

#[derive(PartialEq)]
enum SfpState {
    RECV,
    NEW,
}

#[derive(PartialEq)]
enum SfpEscState {
    NORMAL,
    ESCAPING,
}

pub enum SfpError {
    DataTooShort,
    CrcFailed,
    UnknownHeader,
    Other(&'static str),
}

#[derive(PartialEq)]
pub enum ConnectState {
    DISCONNECTED,
    SENT_SYN0,
    SENT_SYN1,
    CONNECTED,
}

impl Display for SfpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SfpError::DataTooShort => write!(f, "Data too short."),
            SfpError::CrcFailed => write!(f, "CRC check failed."),
            SfpError::UnknownHeader => write!(f, "Unknown header."),
            SfpError::Other(reason) => write!(f, "Other reason: {}", reason),
        }
    }
}

impl fmt::Debug for SfpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", format!("{}", self))
    }
}

type SfpResult<T> = Result<T, SfpError>;

/* Stolen from avr-libc's docs */
/*
static uint16_t _crc_ccitt_update (uint16_t crc, uint8_t octet) {
  octet ^= crc & 0xff;
  octet ^= octet << 4;
  return ((((uint16_t)octet << 8) | ((crc >> 8) & 0xff)) ^ (uint8_t)(octet >> 4) ^ ((uint16_t)octet << 3));
}
*/

fn crc_update(crc: u16, byte: u8) -> u16 {
    let mut b:u16 = byte as u16;
    b ^= crc & 0xff;
    b ^= (b<<4)&0x00ff;
    ((b<<8)|((crc>>8)&0xff)) ^ (b>>4) ^ (b<<3)
}

#[repr(C)]
pub struct Codec {
    header: u8,
    crc: Crc,
    state: SfpState,
    esc_state: SfpEscState,
    buf: Vec<u8>,
    in_buf: Vec<u8>,
}

impl Codec {
    pub fn new() -> Codec {
        Codec {
            header: 0,
            crc: SFP_CRC_INIT,
            state: SfpState::NEW,
            esc_state: SfpEscState::NORMAL,
            buf: Vec::new(),
            in_buf: Vec::new()
        }
    }

    pub fn deliver(&mut self, octet: u8) -> Option<SfpResult<SfpPacket>> {
        self.in_buf.push(octet);
        // Look for a flag in the buffer. If there is no flag, we need to wait for more bytes
        let flag = SFP_FLAG;
        if !self.in_buf.as_slice().contains(&flag) {
            return None;
        }         
        while self.in_buf.len() > 0 {
            //let byte = in_buf.drain_to(1).as_slice()[0];
            let byte = self.in_buf.remove(0);
            match byte {
                SFP_FLAG => {
                    if self.state == SfpState::RECV {
                        // Done receiving the frame.
                        return Some(self.process_frame());
                    } else {
                        self.reset();
                    }
                }
                SFP_ESC => {
                    self.esc_state = SfpEscState::ESCAPING;
                }
                b => {
                    let mut _b = b;
                    if self.esc_state == SfpEscState::ESCAPING {
                        _b = b^SFP_ESC_FLIP_BIT;
                        self.esc_state = SfpEscState::NORMAL;
                    }
                    self.crc_update(_b);
                    if self.state == SfpState::NEW {
                        self.header = _b;
                        self.state = SfpState::RECV;
                    } else {
                        /* Receive a byte. */
                        self.buf.push(_b);
                    }
                }
            }
        }
        return None;
    }

    pub fn encode(&mut self, msg: SfpPacket) -> SfpResult<Vec<u8>> {
        let mut crc:Crc = SFP_CRC_INIT;
        let mut buf = Vec::new();
        buf.push(SFP_FLAG);
        match msg {
            SfpPacket::Usr{seq: _seq, buf: _buf} => {
                let header:u8 = (FRAMETYPE_USR << 6) | _seq;
                crc = crc_update(crc, header);
                if [SFP_ESC, SFP_FLAG].contains(&header) {
                        buf.push(SFP_ESC);
                        buf.push(header^SFP_ESC_FLIP_BIT);
                } else {
                    buf.push(header);
                }
                for byte in _buf {
                    crc = crc_update(crc, byte);
                    if [SFP_ESC, SFP_FLAG].contains(&byte) {
                        buf.push(SFP_ESC);
                        buf.push(byte^SFP_ESC_FLIP_BIT);
                    } else {
                        buf.push(byte);
                    }
                }
            }
            SfpPacket::Rtx{seq: _seq, buf: _buf} => {
                let header:u8 = (FRAMETYPE_RTX << 6) | _seq;
                //info!("Sending RTX SFP packet with header: 0x{:x}", header);
                crc = crc_update(crc, header);
                if [SFP_ESC, SFP_FLAG].contains(&header) {
                        buf.push(SFP_ESC);
                        buf.push(header^SFP_ESC_FLIP_BIT);
                } else {
                    buf.push(header);
                }
                for byte in _buf {
                    crc = crc_update(crc, byte);
                    if [SFP_ESC, SFP_FLAG].contains(&byte) {
                        buf.push(SFP_ESC);
                        buf.push(byte^SFP_ESC_FLIP_BIT);
                    } else {
                        buf.push(byte);
                    }
                }
            }
            SfpPacket::Nak{seq: _seq} => {
                let header:u8 = (FRAMETYPE_NAK << 6) | _seq;
                crc = crc_update(crc, header);
                buf.push(header);
            }
            SfpPacket::Syn{seq: _seq} => {
                let header:u8 = (FRAMETYPE_SYN << 6) | _seq;
                crc = crc_update(crc, header);
                buf.push(header);
            }
        }
        // Push the complement of the crc onto the message
        let crc = !crc;
        let bytes = vec![ (crc & 0x00ff) as u8, ((crc>>8) & 0x00ff) as u8 ];
        for byte in bytes {
            if [SFP_ESC, SFP_FLAG].contains(&byte) {
                buf.push(SFP_ESC);
                buf.push(byte^SFP_ESC_FLIP_BIT);
            } else {
                buf.push(byte);
            }
        }
        // Push the flag
        buf.push(SFP_FLAG);
        Ok(buf)
    }

    fn process_frame(&mut self) -> SfpResult<SfpPacket> {
        if self.buf.len() < mem::size_of::<Crc>() {
            Err( SfpError::DataTooShort )
        }

        else if self.crc != SFP_CRC_GOOD {
            self.soft_reset();
            Err( SfpError::CrcFailed )
        }

        else {
            let len = self.buf.len();
            self.buf.split_off( len-2 );
            let buf = self.buf.clone();
            //info!("SFP Packet Header: 0x{:X}", self.header);
            let rc = match self.header >> 6 {
                FRAMETYPE_USR => SfpPacket::Usr{ seq: self.header&0x3F, buf: buf},
                FRAMETYPE_RTX => SfpPacket::Rtx{ seq: self.header&0x3F, buf: buf},
                FRAMETYPE_NAK => SfpPacket::Nak{ seq: self.header&0x3F },
                FRAMETYPE_SYN => SfpPacket::Syn{ seq: self.header&0x3F },
                _ => { 
                    return Err( SfpError::UnknownHeader ); 
                }
            };
            self.reset();
            Ok(rc)
        }
    }

    pub fn soft_reset(&mut self) {
        self.header = 0;
        self.crc = SFP_CRC_INIT;
        self.state = SfpState::NEW;
        self.esc_state = SfpEscState::NORMAL;
    }

    pub fn reset(&mut self) {
        self.soft_reset();
        self.in_buf.clear();
        self.buf.clear();
    }


    fn crc_update(&mut self, byte: u8) {
        self.crc = crc_update(self.crc, byte);
    }
}

#[repr(C)]
pub struct Context {
    codec: Codec,
    rx_seq: u8,
    tx_seq: u8,
    history: VecDeque<SfpPacket>,
    connect_state: ConnectState,
    deliver_cb: Option<Box<FnMut(&Vec<u8>)>>,
    write_cb: Option<Box<FnMut(&[u8]) -> usize>>,
    connect_cb: Option<Box<FnMut()>>
}

impl Context {
    pub fn new() -> Context {
        Context{ codec: Codec::new(),
                 rx_seq: 0,
                 tx_seq: 0,
                 history: VecDeque::with_capacity(HISTORY_SIZE),
                 connect_state: ConnectState::DISCONNECTED,
                 deliver_cb: None,
                 write_cb: None,
                 connect_cb: None
        }
    }

    pub fn connect(&mut self) -> SfpResult<usize> {
        //! Begin the connection process. Check the connection status periodically to ensure the
        //! context is connected before sending data.
        self.codec.reset();
        self.rx_seq = 0;
        self.tx_seq = 0;
        self.write_packet( SfpPacket::Syn{ seq: SEQ_SYN0 } )
    }

    pub fn connect_state(&self) -> &ConnectState {
        &self.connect_state
    }
    
    pub fn deliver(&mut self, octet: u8) -> SfpResult<Option<Vec<u8>>> {
        //! Deliver octets from the transport layer to this function
        match self.codec.deliver(octet) {
            Some(Ok(packet)) => {
                match packet {
                    SfpPacket::Usr{seq, buf} => {
                        // Check to see that the sequence number is correct
                        if self.rx_seq == seq {
                            // Update our own seq num
                            self.rx_seq = next_seq_num(seq);
                            if let Some(ref mut cb) = self.deliver_cb {
                                cb( &buf );
                            }
                            Ok(Some(buf))
                        } else {
                            // Send nak
                            //warn!("Received out-of-bounds sequence number. Sending NAK...");
                            let seq = self.rx_seq;
                            self.write_packet( SfpPacket::Nak{seq: seq} )
                                .and_then(|_| Ok(None) )
                        }
                    }
                    SfpPacket::Rtx{seq, buf} => {
                        // Check to see that the sequence number is correct
                        if self.rx_seq == seq {
                            // Update our own seq num
                            self.rx_seq = next_seq_num(seq);
                            Ok(Some(buf))
                        } else { // If seq is incorrect, ignore this packet
                            Ok(None)
                        }
                    }
                    SfpPacket::Nak{seq} => {
                        // Find the packet with sequence number 'seq' in our tx history
                        //info!("SFP Received NAK. Delivering repeat packet...");
                        let packet = match self.history.iter().find(|p| {
                            match **p {
                                SfpPacket::Usr{seq: _seq, buf: _} => _seq == seq,
                                _ => false,
                            }
                        }) {
                            Some(packet) => { 
                                let rtx = match packet {
                                    &SfpPacket::Usr{seq, ref buf} =>
                                        SfpPacket::Rtx{seq: seq, buf: buf.clone()},
                                        ref p => (*p).clone()
                                };
                                rtx
                            }
                            _ => {
                                return Err(SfpError::Other("Could not find packet in history!"));
                            }
                        };
                        self.write_packet(packet)
                            .and_then(|_| Ok(None) )
                    }
                    SfpPacket::Syn{seq} => {
                        self.handle_syn(seq).map(|_| None)
                    }
                }
            },
            Some(Err(_)) => {
                // Send a NAK
                self.send_nak().unwrap_or_else(|e| {
                    //warn!("Could not send NAK: {}", e);
                    0
                });
                Ok(None)
            },
            _ => Ok(None)
        }
    }

    fn handle_syn(&mut self, seq: u8) -> SfpResult<()> {
        match seq {
            SEQ_SYN0 => {
                // Send SYN1
                self.connect_state = ConnectState::SENT_SYN1;
                self.write_packet( SfpPacket::Syn{seq: SEQ_SYN1} )
                    .and_then(|_| { Ok(()) } )
            }
            SEQ_SYN1 => {
                // Send SYN2
                self.connect_state = ConnectState::CONNECTED;
                if let Some(ref mut cb) = self.connect_cb {
                    cb();
                }
                self.write_packet( SfpPacket::Syn{seq: SEQ_SYN2} )
                    .and_then(|_| { Ok(()) } )
            }
            SEQ_SYN2 => {
                self.connect_state = ConnectState::CONNECTED;
                Ok(())
            }
            SEQ_SYN_DIS => {
                self.connect_state = ConnectState::DISCONNECTED;
                Ok(())
            }
            _ => {
                Err(SfpError::Other("Received invalid SYN Sequence number."))
            }
        }
    }

    pub fn set_write_callback<F>(&mut self, callback: F) 
        where F: FnMut(&[u8]) -> usize,
              F: 'static
    {
        self.write_cb = Some(Box::new(callback));
    }

    pub fn set_connect_callback<F>(&mut self, callback: F) 
        where F: FnMut(),
              F: 'static
    {
        self.connect_cb = Some(Box::new(callback));
    }

    pub fn set_deliver_callback<F>(&mut self, callback: F)
        where F: FnMut(&Vec<u8>),
              F: 'static
    {
        self.deliver_cb = Some(Box::new(callback));
    }

    pub fn write(&mut self, buf: Vec<u8>) -> SfpResult<usize> {
        let tx_seq = self.tx_seq;
        let packet = SfpPacket::Usr{ seq: tx_seq, buf: buf };
        while self.history.len() >= HISTORY_SIZE {
            self.history.pop_front();
        }
        self.history.push_back(packet.clone());
        let rc = self.write_packet(packet);
        self.tx_seq = next_seq_num(self.tx_seq);
        rc
    }

    fn write_impl(&mut self, buf: &[u8]) -> SfpResult<usize> {
        match self.write_cb {
            Some(ref mut func) => Ok(func(buf)),
            None => Err(SfpError::Other("Error! Write callback not set.")),
        }
    }

    fn write_packet(&mut self, packet: SfpPacket) -> SfpResult<usize> {
        let result = self.codec.encode(packet).unwrap();
        let data = result.as_slice();
        self.write_impl(data)
    }

    fn send_nak(&mut self) -> SfpResult<usize> {
        if self.connect_state != ConnectState::CONNECTED {
            return Err(SfpError::Other("Not sending NAK: not connected."));
        }
        let packet = SfpPacket::Syn{ seq: self.rx_seq };
        self.write_packet(packet)
    }
}

fn next_seq_num(seq: u8) -> u8 {
    let mut n = seq+1;
    if n > 0x3f {
        n = 0;
    }
    n
}

// Unmangled extern "C" functions

/*
#[no_mangle]
pub extern fn sfp_new() -> Box<Context> {
    Box::new(Context::new())
}

#[no_mangle]
pub extern fn test_func(buf: &mut [u8]){
    buf[1] = 666;
}

#[no_mangle]
pub extern "C" fn deliver(ctx: &mut Box<Context>, octet: u8) -> Box<SfpResult<Option<Vec<u8>>>> {
    Box::new(Err(SfpError::Other("Not implemented.")))
}
*/

#[test]
fn it_works() {
}


