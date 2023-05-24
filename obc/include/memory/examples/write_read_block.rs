#![no_main] //Tell the rust compiler Main isn't used.
#![no_std] use core::num::Wrapping;

//Tell the rust compiler we are using the core library.
//Include defferred formatting and global logger.
//Used for Serial monitor out.
use defmt as _;
use defmt_rtt as _;

use embedded_hal::blocking::serial::write;
use flash::w25q128::{Memory, FlashInfo};
//Defines how we should panic -> Using probe-run.
use panic_probe as _;

//Hal for stm32f4 series -> f446re is defined in Cargo.toml
use stm32f4xx_hal as hal;
use hal::{pac::{self}, prelude::*, spi::{Event}};

//Used for setting up project for Cortex M processors.
use cortex_m_rt::entry;

#[entry] //Entry point of the program
fn main()-> ! {
    defmt::info!("Monitor Running");

    //Grab peripherals from Cortex and Hal library.
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();
    let mut dp = pac::Peripherals::take().unwrap();

    //Grabbing RCC from Hal periphals (STM clocks).
    let rcc = dp.RCC.constrain();
    //Setup clock speeds, 8 MHz external osc, 180MHz sysclk, APB2 90MHz
    let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(180.MHz()).pclk2(90.MHz()).freeze();
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
    let miso = gpioa.pa6.into_alternate::<5>().speed(hal::gpio::Speed::VeryHigh);
    let mosi = gpioa.pa7.into_alternate().speed(hal::gpio::Speed::VeryHigh);
    let mut cs = gpiob.pb6.into_push_pull_output(); 
    cs.set_high();//Device active low.
     
    //Settings for SPI mode, Polarity and phase. 
    let spi_mode = hal::spi::Mode {
        polarity: hal::spi::Polarity::IdleLow,
        phase: hal::spi::Phase::CaptureOnFirstTransition,
    };

    let mut spi = dp.SPI1.spi(
        (sclk, miso, mosi), //Settings SPI pins
        spi_mode, //Setting Mode
        10.MHz(), //Setting clock
        &clocks, //Give a reference to system clocks.
    );
    let mut rtc = hal::rtc::Rtc::new_lsi(dp.RTC, &mut dp.PWR);
 

    /*
    Note the given SPI frequency might not be exact, as the HAL, tries to find the 
    one closest to the given value:
    For precise frequency use the prescalers 2, 4, 8, 16, 32, 64, 128, 256
    fpclk / prescaler.
    
    How this is done can be found at: stm32f4xx_hal file spi.rs:501
    */
    
    //Another possibility is SPI bidi, which is for bidirectional using only the MOSI line. 
    // let mut spi = dp.SPI1.spi_bidi(
        //     (sclk, miso, mosi),
        //     spi_mode,
        //     1.MHz(),
        //     &clocks);
        spi.bit_format(hal::spi::BitFormat::MsbFirst); //Set bit_format MSB is standard
        spi.enable(true); //On by default after declaration, but needed after disable
        //spi.enable(false); //Disables SPI, make sure no transmission is occuring.
        spi.listen(Event::Rxne); //Enables hardware interrupt on RXNE. 
        let mut memory = Memory::new_w25q128(spi, cs);
        
        // let address: u32 = {0x1012ff};
        // let output = flash::ws25j128::split_address(address);
        // for i in output {
        //     defmt::info!("{:x}",i);
        // }
        memory.delete(flash::w25q128::Delete::BlockErase64, 0x8000);
        let mut writebuffer:[u8;1024*4] = [0;1024*4];
        let mut smallcount: u8 = 0; 
        for i in 0..1024*4 {
            writebuffer[i] = smallcount;
            if smallcount == 255 {
                smallcount = 0;}
            else {
                smallcount += 1;
            }
        }
        let mut addr = 0x0;
        for i in 0..16 {
            memory.write(addr, &writebuffer);
            addr +=0x1000
        }
        // for i in writebuffer {
        //     defmt::info!("{}",i);
        // }
        // memory.write(0xa000, &writebuffer);
        // // defmt::info!("{}",memory.write(0x001000, &writebuffer));
        let mut addr = 0x0;
        for i in 0..17 {
        let mut data: [u8;1024*4] = [0;1024*4];
        memory.read(addr, 1024*4, &mut data);
        let mut counter =0;
        for i in data {
            defmt::info!("{}: {}",counter, i);
            counter+=1;
        }
        addr += 0x1000;
    }

        
        
    loop {
    // defmt::info!("time: {}", rtc.get_datetime().assume_utc().unix_timestamp());
    // _delay.delay_ms(100_u32);
    }
}
