use serial::prelude::*;
use serial::unix::TTYPort;
use serial::SystemPort;
use std::env;
use std::io;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::time::Duration;

const CHAR_SIZE: serial::CharSize = serial::Bits8;
const PARITY: serial::Parity = serial::ParityNone;
const STOP_BITS: serial::StopBits = serial::Stop1;
const FLOW_CONTROL: serial::FlowControl = serial::FlowNone;
const TIMEOUT: Duration = Duration::from_millis(60);

fn main() {
    let mut port = serial::open("/dev/ttyUSB0").unwrap();
    port.set_timeout(TIMEOUT);
    let settings = serial::PortSettings {
        baud_rate: serial::BaudRate::Baud9600,
        char_size: CHAR_SIZE,
        parity: PARITY,
        stop_bits: STOP_BITS,
        flow_control: FLOW_CONTROL,
    };
    port.configure(&settings).unwrap();
    
    let mut in_buf: Vec<u8> = vec![0; 512];
    let mut in_data: Vec<u8> = Vec::new();
    loop {
        
        match port.read(in_buf.as_mut_slice()) {
            Ok(n) if {n > 0} => {
                in_data.append(in_buf[0..n].to_vec().as_mut());
            }
            _ if {in_data.len() > 0} => {
                println!("[ DEBUG ] UART data in {:?}", &in_data);
                in_data.clear();
            }
            _ => {}
        };
        let sleep: Duration = Duration::from_millis(60);
        std::thread::sleep(sleep);
    }
}
