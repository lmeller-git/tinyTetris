#![no_std]
#![no_main]

extern crate alloc;

use libtinyos::{println, process::ProcessError};

use crate::{game::game_loop, graphics::init_gfx};

mod game;
mod graphics;
mod interface;

#[unsafe(no_mangle)]
pub fn main() -> Result<(), ProcessError> {
    println!("Welcome to TinyTetris.\nLaunching the game...");
    init_gfx();
    game_loop();
    Ok(())
}
