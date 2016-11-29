extern crate mimar;

use std::io::BufReader;
use std::fs::File;
use std::env;

use mimar::{Mima, MimaState};
use mimar::firmware::Firmware;

fn main() {
    let firmware = env::args().nth(1).unwrap();
    let filename = env::args().nth(2).unwrap();
    let fw = File::open(firmware).unwrap();
    let file = File::open(filename).unwrap();
    let mut buf_fw = BufReader::new(fw);
    let mut buf_reader = BufReader::new(file);
    let mut m = Mima::new();
    m.firmware = Firmware::load(&mut buf_fw).unwrap();
    m.load(&mut buf_reader).unwrap();
    m.jump(0x100);
    println!("{:#?}", m);
    loop {
        let state = m.cycle();
        if state != MimaState::Running {
            println!("{:?}", state);
            break;
        }
    };
    println!("{:#?}", m);
}
