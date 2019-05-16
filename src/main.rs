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

struct UARTConfig {
    serial_port: String,
    nos_bus: String,
}

fn main() {
    let mut i2cs: HashMap<String, I2CConfig> = HashMap::new(); 
    let mut uarts: HashMap<String, UARTConfig> = HashMap::new(); 
    
    // Initialize config
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

    // Register all uart configurations
    for entry in uart_table.iter() {
        let name = entry.0;
        let config = entry.1.clone();
        let mut entry_table = config.try_into::<toml::value::Table>().unwrap();

        let serial_port = entry_table
            .remove("serial_port")
            .ok_or(std::io::Error::new(
                ErrorKind::Other,
                "Error parsing config.toml",
            ))
            .unwrap()
            .try_into::<String>()
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
        
        println!(
            "<config: UART {} => serial_port '{}', nos_bus '{}'",
            &name, &serial_port, &nos_bus
        );

        let config = UARTConfig {
            serial_port,
            nos_bus,
        };

        uarts.insert(name.to_string(), config);
    }
    
    // Register all i2c configurations
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
            "<config: I2C {} => device_path '{}', slave_address '{}'",
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
    
    // Main program loop
    loop {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(n) => match input.split_whitespace().next() {
                Some(cmd) => match cmd {
                    "help" => {
                        println!(
                            "<commands: 'uart', 'i2c'"
                        );
                    }
                    "uart" => {
                        if let Some(arg) = input.split_whitespace().nth(1) {
                            let arg = arg.trim();
                            match arg {
                                "all" => {
                                    if uarts.is_empty() {
                                        println!("<uart: error => no uart configs are available");
                                        continue;    
                                    }
                                    for (name, config) in uarts.iter() {
                                        println!(
                                            "<uart: starting uart {} => if it hangs, restart NOS3 and nos3_io", 
                                            &name
                                        );
                                        let serial_port = config.serial_port.to_string();
                                        let nos_bus = config.nos_bus.to_string();
                                        thread::spawn(move || {
                                            uart_init(serial_port, nos_bus);
                                        });
                                    }
                                    uarts.clear();
                                }
                                _ => {
                                    if (!uarts.contains_key(arg)) {
                                        println!("<uart: error => uart config not available");
                                        continue;
                                    }
                                    let config = uarts.get(arg).unwrap();
                                    println!(
                                        "<uart: starting uart {} => if it hangs, restart NOS3 and nos3_io", 
                                        &arg
                                    );
                                    let serial_port = config.serial_port.to_string();
                                    let nos_bus = config.nos_bus.to_string();
                                    thread::spawn(move || {
                                            uart_init(serial_port, nos_bus);
                                    });
                                    uarts.remove(arg);
                                }
                            }
                        } else {
                            println!("<help: 'uart all', 'uart [name]'");
                        }
                    }
                    "i2c" => {
                        if let Some(arg) = input.split_whitespace().nth(1) {
                            let arg = arg.trim();
                            match arg {
                                "all" => {
                                    if i2cs.is_empty() {
                                        println!("<i2c: error => no i2c configs are available");
                                        continue;    
                                    }
                                    for (name, config) in i2cs.iter() {
                                        println!("<i2c: starting i2c {}", &name);
                                        let path = config.device_path.to_string();
                                        let addr = config.slave_address;
                                        let bus = config.nos_bus.to_string();
                                        let nos_slave = config.nos_slave_addr;
                                        thread::spawn(move || {
                                            i2c_init(path, addr, bus, nos_slave);
                                        });
                                    }
                                    i2cs.clear();
                                }
                                _ => {
                                    if (!i2cs.contains_key(arg)) {
                                        println!("<i2c: error => i2c config not available");
                                        continue;
                                    }
                                    let config = i2cs.get(arg).unwrap();
                                    println!("<i2c: starting i2c {}", &arg);
                                    let path = config.device_path.to_string();
                                    let addr = config.slave_address;
                                    let bus = config.nos_bus.to_string();
                                    let nos_slave = config.nos_slave_addr;
                                    thread::spawn(move || {
                                        i2c_init(path, addr, bus, nos_slave);
                                    });
                                    i2cs.remove(arg);
                                }
                            }
                            //println!("Starting I2C forwarding and receiving on '{}'", &path_arg);
                        } else {
                            println!("<help: 'i2c all', 'i2c [name]'");
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

fn uart_init(serial_port: String, nos_bus: String) {
    // TODO: get nos connection string from config
    let mut uart = match UART::new("fsw", "tcp://localhost:12000", nos_bus.as_str(), 1) {
        Ok(uart) => {
            println!("Established UART connection to NOS! Starting...");
            uart
        },
        Err(e) => {
            println!("NOS connection failure. Try restarting NOS3");
            return;
        }
    };

    let mut port = match serial::open(serial_port.as_str()) {
        Ok(port) => port,
        Err(err) => {
            println!(
                "Error opening the serial port, details: {}",
                err
            );
            return;
        }
    };

    port.set_timeout(Duration::from_millis(100));
    let settings = serial::PortSettings {
        baud_rate: serial::BaudRate::Baud9600,
        char_size: CHAR_SIZE,
        parity: PARITY,
        stop_bits: STOP_BITS,
        flow_control: FLOW_CONTROL,
    };
    port.configure(&settings).unwrap();

    let mut in_buf: Vec<u8> = vec![0; 512]; // transient incoming data read, differs each loop iteration
    let mut in_data: Vec<u8> = Vec::new(); // entire data block to write to NOS (stringed together from in_bufs)
    
    // Keep this thread working for the lifetime of the program
    loop {
        // incoming UART data to NOS UART
        match port.read(in_buf.as_mut_slice()) {
            Ok(n) if {n > 0} => {
                in_data.append(in_buf[0..n].to_vec().as_mut());
            }
            _ if {in_data.len() > 0} => {
                uart.write(&in_data);
                //println!("[ DEBUG ] UART data in {:?}", &in_data);
                in_data.clear();
            }
            _ => {}
        };
        // outgoing NOS data to UART
        let data = uart.read(512);
        match &data.len() {
            0 => {},
            _ => {
                let out = data.as_slice();
                //println!("[ DEBUG ] UART data out [{}...{}]", &out[0], &out[data.len() as usize - 1]);
                port.write_all(out);
            },
        };
    }
}

fn i2c_init(path: String, addr: u16, nos_bus: String, nos_slave_addr: u16) {
    // TODO: get nos connection string from config
    let nos_i2c = match I2CMaster::new(119, "tcp://localhost:12000", nos_bus.as_str()) {
        Ok(i2c) => {
            println!("Established I2C connection to NOS! Starting...");
            i2c
        },
        Err(e) => {
            println!("NOS connection failure. Try restarting NOS3");
            return;
        }
    };

    let mut i2c = I2c::from_path(path.clone()).unwrap(); // real i2c device
    let mut last_cmd: u8 = 0x00;  // keep track of the last cmd written to sim, so when the external device wants to read the response, we can tell the sim what command
    // NOTE: linux_i2c cannot read or write block data more than 32 bytes (according to documentation). hopefully that's a non-issue?
    
    // Keep this thread working for the lifetime of the program
    loop {
        // outgoing NOS data to I2C
        match nos_i2c.read(nos_slave_addr, 32) {
            Ok(data) => {
                match &data.len() {
                    0 => {},
                    _ => {
                        i2c.smbus_set_slave_address(addr, false).unwrap();
                        // data[0] might be the command byte
                        // if so, replace cmd from last write with data[0] 
                        i2c.i2c_write_block_data(last_cmd, data.as_slice());
                    },
                }
            }
            _ => {}
        }
        // incoming I2C data to NOS        
        let mut data = vec![0; 32];
        i2c.smbus_set_slave_address(addr, false).unwrap();
        let cmd = match i2c.smbus_read_byte() {
            Ok(cmd) => {
                match i2c.i2c_read_block_data(cmd, &mut data) {
                    Ok(n) if {n > 0} => {
                        last_cmd = cmd;
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

