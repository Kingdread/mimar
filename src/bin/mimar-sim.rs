extern crate mimar;
extern crate rustc_serialize;
extern crate docopt;

use std::io::BufReader;
use std::fs::File;
use std::process;
use std::fmt::Write;

use docopt::Docopt;

use mimar::{Mima, MimaState};
use mimar::firmware::{Instruction, Firmware};
use mimar::logger::Logger;
use mimar::util;

struct ConsoleLogger;

impl Logger for ConsoleLogger {
    fn log_instruction(&self, m: &Mima, iar: u32, instr: &Instruction, param: u32) {
        let mut labels = m.find_labels(iar).into_iter();
        let label = labels.next().unwrap_or("");
        let mut param_label = String::new();
        let mut param_labels = m.find_labels(param).into_iter();
        // only show the label if it's not LDC and not an extended instruction
        if instr.opcode > 0 && instr.opcode <= 0xF {
            if let Some(label) = param_labels.next() {
                write!(param_label, " ({})", label).unwrap();
            }
        }
        println!("{:>6} [{:#08x}] {:>10} ({:#04x})[{:^7}] {:#8x}{}",
                 m.cycle_count, iar, label, instr.opcode, instr.mnemonic,
                 param, param_label);
    }
}

const USAGE: &'static str = "
MIMA simulator.

Usage:
  mimar-sim [-s <loc>] <firmware> <input>
  mimar-sim -h | --help

Options:
  firmware                  Firmware file (compiled with mimar-fwc)
  input                     Program to execute (assembled with mimar-asm)
  -s <loc>, --start <loc>   Start location, given as number or label.
  -h --help                 Show this screen.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    flag_start: Option<String>,
    arg_firmware: String,
    arg_input: String,
}

fn file_input(name: &str) -> BufReader<File> {
    let f = File::open(name).unwrap_or_else(|e| {
        println!("Can't open {}: {}", name, e);
        process::exit(1);
    });
    BufReader::new(f)
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let mut m = Mima::new();
    m.firmware = Firmware::load(&mut file_input(&args.arg_firmware))
        .unwrap_or_else(|e| {
            println!("Error loading the firmware: {}", e);
            process::exit(1);
        });
    m.load(&mut file_input(&args.arg_input)).unwrap_or_else(|e| {
        println!("Error loading the program: {}", e);
        process::exit(1);
    });

    if let Some(start) = args.flag_start {
        let num = util::parse_num(&start)
            .map(|v| v as u32)
            .or_else(|| m.labels.get(&start).map(|v| *v as u32))
            .unwrap_or_else(|| {
                println!("Can't find start {}", start);
                process::exit(1);
            });
        m.jump(num);
    }

    loop {
        let state = m.cycle(&ConsoleLogger);
        if state != MimaState::Running {
            println!("{:?}", state);
            break;
        }
    };

    let mut labels = m.labels.iter().collect::<Vec<_>>();
    labels.sort_by_key(|&(_, adr)| *adr);
    for (label, address) in labels {
        let data = m.get_memory(*address);
        println!("  Cell {:#08x} {:>10}: {:#8x} ({})",
                 address, label, data, data);
    }
}
