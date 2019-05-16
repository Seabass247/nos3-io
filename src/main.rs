extern crate serial;

use i2c_linux::I2c;
use nosengine_rust::client::i2c::I2CMaster;
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
use toml::map::Map;
use std::collections::HashMap;

const CHAR_SIZE: serial::CharSize = serial::Bits8;
const PARITY: serial::Parity = serial::ParityNone;
const STOP_BITS: serial::StopBits = serial::Stop1;
const FLOW_CONTROL: serial::FlowControl = serial::FlowNone;
const TIMEOUT: Duration = Duration::from_millis(60);

struct I2CConfig {
    device_path: String,
    slave_address: u16,
    nos_bus: String,
    nos_slave_addr: u16,
}

fn main() {
    let mut i2cs: HashMap<String, I2CConfig> = HashMap::new(); 
    
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

    let mut nos_uart_bus = uart_table
        .remove("nos_bus")
        .ok_or(std::io::Error::new(
            ErrorKind::Other,
            "Error parsing config.toml",
        ))
        .unwrap()
        .try_into::<String>()
        .unwrap();        

    println!(
        "<config: UART => serial port '{}'",
        &uart_serial_port,
    );

    for entry in i2c_table.iter() {
        let name = entry.0;
        let config = entry.1.clone();
        let mut entry_table = config.try_into::<toml::value::Table>().unwrap();
        let device_path = entry_table
            .remove("device_path")
            .ok_or(std::io::Error::new(
                ErrorKind::Other,
                "Error parsing config.toml",
            ))
            .unwrap()
            .try_into::<String>()
            .unwrap();
        let slave_address = entry_table
            .remove("slave_address")
            .ok_or(std::io::Error::new(
                ErrorKind::Other,
                "Error parsing config.toml",
            ))
            .unwrap()
            .try_into::<u16>()
            .unwrap();                     

        let nos_bus = entry_table
            .remove("nos_bus")
            .ok_or(std::io::Error::new(
                ErrorKind::Other,
                "Error parsing config.toml",
            ))
            .unwrap()
            .try_into::<String>()
            .unwrap();       

        let nos_slave_addr = entry_table
            .remove("nos_slave_addr")
            .ok_or(std::io::Error::new(
                ErrorKind::Other,
                "Error parsing config.toml",
            ))
            .unwrap()
            .try_into::<u16>()
            .unwrap();      

        println!(
            "<config: I2C '{}' => device path '{}', slave address '{}'",
            &name, &device_path, &slave_address
        );

        let config = I2CConfig {
            device_path,
            slave_address,
            nos_bus,
            nos_slave_addr,
        };

        i2cs.insert(name.to_string(), config);                                  
    }

    println!("<help: type 'help' for commands...");
    // TODO: support multiple instances of uart forwarding threads; make Vec of this
    let mut uart_tx: Option<mpsc::Sender<()>> = None;
    
    loop {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(n) => match input.split_whitespace().next() {
                Some(cmd) => match cmd {
                    "help" => {
                        println!(
                            "<commands: 'uart', or 'i2c', or 'stop'"
                        );
                    }
                    "uart" => {
                        println!(
                            "Starting UART forwarding and receiving,{}",
                            "\nif the program hangs, it's probably a NOS issue, restart NOS3. 
                        ");
                        let (tx, rx) = mpsc::channel();
                        let serial_port = uart_serial_port.clone();
                        let nos_bus = nos_uart_bus.clone();
                        uart_tx = Some(tx);
                        thread::spawn(move || {
                            uart_init(serial_port, nos_bus, rx);                
                        });
                    }
                    "i2c" => {
                        if let Some(arg) = input.split_whitespace().nth(1) {
                            let arg = arg.trim();
                            match arg {
                                "all" => {
                                    for (name, config) in i2cs.iter() {
                                        println!("<i2c: starting i2c '{}'", &name);
                                        let path = config.device_path.to_string();
                                        let addr = config.slave_address;
                                        let bus = config.nos_bus.to_string();
                                        let nos_slave = config.nos_slave_addr;
                                        thread::spawn(move || {
                                            i2c_init(path, addr, bus, nos_slave);
                                        });
                                    }
                                }
                                _ => {
                                    if (!i2cs.contains_key(arg)) {
                                        println!("<i2c: error => that i2c config does not exist");
                                        continue;
                                    }
                                    let config = i2cs.get(arg).unwrap();
                                    println!("<i2c: starting i2c '{}'", &arg);
                                    let path = config.device_path.to_string();
                                    let addr = config.slave_address;
                                    let bus = config.nos_bus.to_string();
                                    let nos_slave = config.nos_slave_addr;
                                    thread::spawn(move || {
                                        i2c_init(path, addr, bus, nos_slave);
                                    });
                                }
                            }
                            //println!("Starting I2C forwarding and receiving on '{}'", &path_arg);
                        } else {
                            println!("<help: 'i2c all', 'i2c [name]'");
                        }
                    }
                    "stop" => {
                        if let Some(tx) = &uart_tx {
                            tx.send(());
                        }
                    }
                    _ => {
                        println!("<unknown command! try 'help'");
                    }
                },
                None => {
                    println!("<unknown command! try 'help'");
                }
            },
            Err(error) => println!("error: {}", error),
        }
    }
}

fn uart_init(uart_serial_port: String, nos_bus: String, rx: mpsc::Receiver<()>) {

        let mut uart = match UART::new("fsw", "tcp://localhost:12000", nos_bus.as_str(), 1) {
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

fn i2c_init(path: String, addr: u16, nos_bus: String, nos_slave_addr: u16) {
    
    let nos_i2c = match I2CMaster::new(119, "tcp://localhost:12000", nos_bus.as_str()) {
        Ok(i2c) => i2c,
        Err(e) => {
            println!("NOS connection failure. Try restarting NOS3");
            return;
        }
    };

    let mut i2c = I2c::from_path(path.clone()).unwrap();
    loop {
        // NOS out to I2c
        match nos_i2c.read(nos_slave_addr, 32) {
            Ok(data) => {
                match &data.len() {
                    0 => {},
                    _ => {
                        i2c.smbus_set_slave_address(addr, false).unwrap();
                        // data[0] might be the 1st byte of data, not cmd
                        // if so, replace data[0] with saved cmd from last registered write (below) 
                        i2c.i2c_write_block_data(data[0], &data[1..]);
                    },
                }
            }
            _ => {}
        }
        // I2c in to NOS        
        let mut data = vec![0; 32];
        i2c.smbus_set_slave_address(addr, false).unwrap();
        let cmd = match i2c.smbus_read_byte() {
            Ok(cmd) => {
                match i2c.i2c_read_block_data(cmd, &mut data) {
                    Ok(n) if {n > 0} => {
                        let data = data.as_slice();
                        let cmd = &[cmd];
                        let comm: Vec<_> = cmd.iter().chain(data).cloned().collect();
                        nos_i2c.write(nos_slave_addr, &comm).unwrap();
                    }
                    Ok(_) => {
                        nos_i2c.write(nos_slave_addr, &[cmd]).unwrap();
                    }
                    _ => {}
                }
            },
            _ => {}
        };
    }
}

