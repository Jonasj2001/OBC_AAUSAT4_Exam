#![no_main] //Tell the rust compiler Main isn't used.
#![no_std]
use core::{mem, num::Wrapping};

use cortex_m::register::primask::read;
//Tell the rust compiler we are using the core library.
//Include defferred formatting and global logger.
//Used for Serial monitor out.
use defmt as _;
use defmt_rtt as _;

use embedded_hal::blocking::serial::write;
use flash::w25q128::{FlashInfo, Memory};
//Defines how we should panic -> Using probe-run.
use panic_probe as _;

//Hal for stm32f4 series -> f446re is defined in Cargo.toml
use hal::{
    flash::FlashSector,
    pac::{self},
    prelude::*,
    spi::{Event, Instance},
};
use stm32f4xx_hal as hal;

//Used for setting up project for Cortex M processors.
use cortex_m_rt::entry;

#[entry] //Entry point of the program
fn main() -> ! {
    defmt::info!("Monitor Running");

    //Grab peripherals from Cortex and Hal library.
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();
    let mut dp = pac::Peripherals::take().unwrap();

    //Grabbing RCC from Hal periphals (STM clocks).
    let rcc = dp.RCC.constrain();
    //Setup clock speeds, 8 MHz external osc, 180MHz sysclk, APB2 90MHz
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(180.MHz())
        .pclk2(90.MHz())
        .freeze();
    //Printing clocks speeds:
    defmt::info!("pclk1 is running at: {}", clocks.pclk1().raw());
    defmt::info!("Sysclock is running at: {}", clocks.sysclk().raw());
    //Setup the possibility for blocking delays.
    let mut _delay = cp.SYST.delay(&clocks);

    let gpioa = dp.GPIOA.split(); //Splitting GPIOA into individual pins.
    let gpiob = dp.GPIOB.split();
    //Declaring pins for SPI1 controller, into their corresponding mode.
    //NOTE: sclk, miso and mosi needs to be put into alternate, so they can use the SPI hardware controller.
    let sclk = gpioa.pa5.into_alternate().speed(hal::gpio::Speed::VeryHigh);
    let miso = gpioa
        .pa6
        .into_alternate::<5>()
        .speed(hal::gpio::Speed::VeryHigh);
    let mosi = gpioa.pa7.into_alternate().speed(hal::gpio::Speed::VeryHigh);
    let mut cs = gpiob.pb6.into_push_pull_output();
    cs.set_high(); //Device active low.

    //Settings for SPI mode, Polarity and phase.
    let spi_mode = hal::spi::Mode {
        polarity: hal::spi::Polarity::IdleLow,
        phase: hal::spi::Phase::CaptureOnFirstTransition,
    };

    let mut spi = dp.SPI1.spi(
        (sclk, miso, mosi), //Settings SPI pins
        spi_mode,           //Setting Mode
        10.MHz(),           //Setting clock
        &clocks,            //Give a reference to system clocks.
    );
    let mut memory = Memory::new_w25q128(spi, cs);
    // let sectorsize = 256*16;
    let sectorsize = memory.get_info_sectorsize();
    // let address: u32 = {0x1012ff};
    // let output = flash::ws25j128::split_address(address);
    // for i in output {
    //     defmt::info!("{:x}",i);
    // }
    #[cfg(feature = "clean")]
    memory.delete(flash::w25q128::Delete::BlockErase64, 0x0);

    #[cfg(not(feature = "nowrite"))]
    {
        // set_executed_bytes(&mut memory);
        fill(&mut memory);
    }

    dump_fp(&mut memory);
    defmt::info!("Done");

    loop {
        // defmt::info!("time: {}", rtc.get_datetime().assume_utc().unix_timestamp());
        // _delay.delay_ms(100_u32);
    }
}
const START_ADDR: u32 = 0x0;
const TASK_NUM: u8 = 48;
const TASK_SIZE: u16 = 256;

fn set_executed_bytes<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
) {
    let index = 2;
    let mut addr = START_ADDR + index;
    let scheduled: u8 = 0xf;
    let empty: u8 = 0xff;
    let executed: u8 = 0x9;
    let data = [
        executed, scheduled, executed, scheduled, executed, scheduled, executed, executed,
    ];

    let mut counter = 0;
    for i in data {
        flash.write(addr, &[i]);
        addr += TASK_SIZE as u32;
        counter += 1;
    }
    while counter < TASK_NUM {
        flash.write(addr, &[scheduled]);
        addr += TASK_SIZE as u32;
        counter += 1;
    }
}
fn fill<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
) {
    let index = 2;
    let mut addr = START_ADDR;
    let scheduled: u8 = 0xf;
    let empty: u8 = 0xff;
    let executed: u8 = 0x9;
    let data = [
        executed, scheduled, executed, scheduled, executed, scheduled, executed, executed,
        scheduled, scheduled, scheduled, scheduled, scheduled, scheduled, scheduled, scheduled,
        executed,
    ];
    let mut buffer = [0u8; 256];
    for i in 0..=255 {
        buffer[i] = i as u8;
    }
    let mut counter = 0;
    for i in data {
        buffer[2] = i;
        flash.write(addr, &buffer);
        addr += TASK_SIZE as u32;
        counter += 1;
    }
    while counter < TASK_NUM {
        buffer[2] = scheduled;
        flash.write(addr, &buffer);
        addr += TASK_SIZE as u32;
        counter += 1;
    }
}

fn dump_fp<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
) {
    // let len = TASK_SIZE * TASK_NUM as u16;
    let len = TASK_SIZE * 17;
    let mut addr = START_ADDR;
    let mut addr_cnt = 0;
    while addr_cnt < len {
        let mut data = [0u8; 4096];
        flash.read(addr, 4096, &mut data);
        for i in data {
            defmt::info!("addr: {:x}, data: {:x}", addr_cnt, i);
            addr_cnt += 1;
        }
        addr += 4096;
    }
}
