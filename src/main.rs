extern crate serial;

use nosengine_rust::client::uart::*;
use serial::prelude::*;
use serial::unix::TTYPort;
use serial::SystemPort;
use std::env;
use std::io;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::path::Path;
use std::thread;
use std::sync::mpsc::{self, TryRecvError, Sender, Receiver};
use std::time::Duration;
use toml;

const CHAR_SIZE: serial::CharSize = serial::Bits8;
const PARITY: serial::Parity = serial::ParityNone;
const STOP_BITS: serial::StopBits = serial::Stop1;
const FLOW_CONTROL: serial::FlowControl = serial::FlowNone;
const TIMEOUT: Duration = Duration::from_millis(60);

fn main() {
    //let mut port = TTYPort::open(Path::new("/dev/ttyUSB0")).unwrap();
    //port.set_timeout(Duration::from_millis(100));

    let mut config = (include_str!("./config.toml"))
        .parse::<toml::Value>()
        .unwrap()
        .try_into::<toml::value::Table>()
        .unwrap();

    let mut uart_table = config
        .remove("uart")
        .ok_or(std::io::Error::new(
            ErrorKind::Other,
            "Error parsing config.toml",
        ))
        .unwrap()
        .try_into::<toml::value::Table>()
        .unwrap();

    let mut i2c_table = config
        .remove("i2c")
        .ok_or(std::io::Error::new(
            ErrorKind::Other,
            "Error parsing config.toml",
        ))
        .unwrap()
        .try_into::<toml::value::Table>()
        .unwrap();

    let mut uart_serial_port = uart_table
        .remove("serial_port")
        .ok_or(std::io::Error::new(
            ErrorKind::Other,
            "Error parsing config.toml",
        ))
        .unwrap()
        .try_into::<String>()
        .unwrap();
    
    let i2c_adapter_device = i2c_table
        .remove("adapter_device")
        .ok_or(std::io::Error::new(
            ErrorKind::Other,
            "Error parsing config.toml",
        ))
        .unwrap()
        .try_into::<String>()
        .unwrap();

    println!(
        "Config: UART set to '{}', I2C to '{}'\ntype 'help' for commands...",
        &uart_serial_port, &i2c_adapter_device,
    );

    // TODO: support multiple instances of uart forwarding threads; make Vec of this
    let mut uart_tx: Option<mpsc::Sender<()>> = None;
    
    loop {
        let mut input = String::new();
        print!(">");
        match io::stdin().read_line(&mut input) {
            Ok(n) => match input.split_whitespace().next() {
                Some(cmd) => match cmd {
                    "help" => {
                        println!(
                            "commands: 'uart', or 'i2c', or 'stop'"
                        );
                    }
                    "uart" => {
                        println!(
                            "Starting UART forwarding and receiving,{}",
                            "\nif the program hangs, it's probably a NOS issue, restart NOS3. 
                        ");
                        let (tx, rx) = mpsc::channel();
                        let serial_port = uart_serial_port.clone();
                        uart_tx = Some(tx);
                        thread::spawn(move || {
                            uart_init(serial_port, rx);                
                        });
                    }
                    "i2c" => {
                        println!("not yet implemented!");
                    }
                    "stop" => {
                        if let Some(tx) = &uart_tx {
                            tx.send(());
                        }
                    }
                    _ => {
                        println!("unknown command! try 'help'");
                    }
                },
                None => {
                    println!("unknown command! try 'help'");
                }
            },
            Err(error) => println!("error: {}", error),
        }
    }
}

fn uart_init(uart_serial_port: String, rx: mpsc::Receiver<()>) {

        let mut uart = match UART::new("fsw", "tcp://localhost:12000", "usart_1", 1) {
            Ok(uart) => uart,
            Err(e) => {
                println!("NOS connection failure. Try restarting NOS3");
                return;
            }
        };
        //println!("Established NOS connection! Forwarding to '{}'...", &serial);
        let mut port = serial::open(uart_serial_port.as_str()).unwrap();
        port.set_timeout(Duration::from_millis(100));
        let settings = serial::PortSettings {
            baud_rate: serial::BaudRate::Baud9600,
            char_size: CHAR_SIZE,
            parity: PARITY,
            stop_bits: STOP_BITS,
            flow_control: FLOW_CONTROL,
        };
        port.configure(&settings).unwrap();

        // Thread control loop, stops when you send `()` to its `rx` channel end point
        let mut in_buf: Vec<u8> = vec![0; 512];
        let mut in_data: Vec<u8> = Vec::new();
        loop {
            match rx.try_recv() {
                    Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                        println!("UART Stopped.");
                        break;
                    },
                    Err(mpsc::TryRecvError::Empty) => {
                        // Forward incoming UART data in to NOS UART
                        match port.read(in_buf.as_mut_slice()) {
                            Ok(n) if {n > 0} => {
                                in_data.append(in_buf[0..n].to_vec().as_mut());
                            }
                            _ if {in_data.len() > 0} => {
                                uart.write(&in_data);
                                println!("[ DEBUG ] UART data in {:?}", &in_data);
                                in_data.clear();
                            }
                            _ => {}
                        };
                        // Forward NOS UART data out to UART
                        let data = uart.read(512);
                        match &data.len() {
                            0 => {},
                            _ => {
                                let out = data.as_slice();
                                println!("[ DEBUG ] UART data out [{}...{}]", &out[0], &out[data.len() as usize - 1]);
                                port.write_all(out);
                            },
                        }
                    }
            }
        }

}

