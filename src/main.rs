#![no_std]
#![no_main]

extern crate alloc;

use core::hint::spin_loop;

use libtinyos::{exit, print, println};

use crate::interface::query_keyboard_once;

mod game;
mod graphics;
mod interface;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    println!("now printing stuff");
    let mut buf: [u8; 20] = [0; 20];
    loop {
        for code in query_keyboard_once(&mut buf) {
            print!("{code}");
        }
        spin_loop();
    }
    #[allow(unreachable_code)]
    exit(0);
}
