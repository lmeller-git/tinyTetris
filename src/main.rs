#![no_std]
#![no_main]

extern crate alloc;

use libtinyos::{println, syscalls};

use crate::{game::game_loop, graphics::init_gfx};

mod game;
mod graphics;
mod interface;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    println!("Welcome to TinyTetris.\nLaunching the game...");
    init_gfx();
    game_loop();
    #[allow(unreachable_code)]
    unsafe {
        syscalls::exit(0);
    }
}
