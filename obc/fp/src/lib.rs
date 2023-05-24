#![no_std]
use defmt as _;
use defmt_rtt as _; // global logger
use fugit as _;
use panic_probe as _; // panic handler
use stm32f4xx_hal as _; // memory layout // time abstractions

//Tilføjer excan som modul;
pub mod excan;
pub mod exrtc;
pub mod flightplanner;
