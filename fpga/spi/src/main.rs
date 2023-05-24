#![no_main] //Tell the rust compiler Main isn't used.
#![no_std] //Tell the rust compiler we are using the core library.
//Include defferred formatting and global logger.
//Used for Serial monitor out.
use defmt as _;
use defmt_rtt as _;

use embedded_hal::digital::v2::IoPin;
//Defines how we should panic -> Using probe-run.
use panic_probe as _;

//Hal for stm32f4 series -> f446re is defined in Cargo.toml
use stm32f4xx_hal as hal;
use hal::{pac::{self}, prelude::*, spi::{Event}, gpio::PinState};

//Used for setting up project for Cortex M processors.
use cortex_m_rt::entry;
pub mod fpga_in;
pub mod fpga_out;
#[entry] //Entry point of the program
fn main()-> ! {
    defmt::info!("Monitor Running");

    //Grab peripherals from Cortex and Hal library.
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    //Grabbing RCC from Hal periphals (STM clocks).
    let rcc = dp.RCC.constrain();
    //Setup clock speeds, AHB1 = 180MHz, APB1 = 45MHz MAX 
    let clocks = rcc.cfgr.use_hse(180.MHz()).pclk1(45.MHz()).pclk2(45.MHz()).freeze();
    //Printing clocks speeds:
    defmt::info!("pclk1 is running at: {}", clocks.pclk1().raw());
    defmt::info!("Sysclock is running at: {}", clocks.sysclk().raw());
    //Setup the possibility for blocking delays.
    let mut _delay = cp.SYST.delay(&clocks);

    let gpioa = dp.GPIOA.split(); //Splitting GPIOA into individual pins.
    let gpiob = dp.GPIOB.split();
    //Declaring pins for SPI1 controller, into their corresponding mode.
    //NOTE: sclk, miso and mosi needs to be put into alternate, so they can use the SPI hardware controller.
    let sclk = gpioa.pa5.into_alternate();
    let miso = gpioa.pa6.into_alternate::<5>();
    let mosi = gpioa.pa7.into_alternate();
    let mut cs = gpioa.pa9.into_push_pull_output();

    let sclk2 = gpiob.pb10.into_alternate::<5>();
    let miso2 = gpiob.pb14.into_alternate::<5>();

    cs.set_high();//Device active low.
     
    //Settings for SPI mode, Polarity and phase. 
    let spi_mode = hal::spi::Mode {
        polarity: hal::spi::Polarity::IdleLow,
        phase: hal::spi::Phase::CaptureOnFirstTransition,
    };

    let mut spi = dp.SPI1.spi(
        (sclk, miso, mosi), //Settings SPI pins
        spi_mode, //Setting Mode
        1.MHz(), //Setting clock
        &clocks, //Give a reference to system clocks.
    );

    let mut spi2 = dp.SPI2.spi(
        (sclk2, miso2, stm32f4xx_hal::gpio::NoPin),
        spi_mode, 
        1.MHz(), 
        &clocks
    );
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

    //A few OPCODES for Winbond W25Q128JV
    //let callsign: [u8;6] = [0x4f, 0x5a, 0x36, 0x43, 0x55, 0x42];
    //For OPCODES, that return values, remember to send trailing 0x0 afterwards.
    
    //cs.set_low();
    //spi.write(&callsign).unwrap(); //spi.write() discards any incoming transmissions.
    //cs.set_high(); //End transmission


    //Example of externalising SPI
    // let mut winbond = spi_external::HalSpi::new(spi, cs, embedded_hal::digital::v2::PinState::Low );
    // winbond.init(); //Setting CS to its non active state.
    // winbond.manid(); //Read W25Q128JV Manufacturerid.
   
    // let mut output = fpga_in::FpgaIn::new(spi, cs);
    let mut flag = gpioa.pa2.into_pull_down_input();
    let mut fsm = gpioa.pa3.into_pull_down_input();


    let mut input = fpga_out::FpgaOut::new(spi2, flag, fsm);
    let mut output = fpga_in::FpgaIn::new(spi,cs);   
    //let mut data = [0u8; 2000];
    //input.read(&mut data);

    let mut data = [0u8; 250];
    let mut storeThisData = [0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,
    0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55,0x55];
    //input.read(&mut data[124..512]);
    // input.readFlag();
    let mut storeThisData = [0u8;256];
    for i in 0..256 {
        storeThisData[i] = i as u8;
    }
    output.ShortCallsign();
    output.write(&storeThisData);

    //hvis data er klar, læs den rigtige mænge data. 
    let mut checkmaker: u8 = 0;
    if input.readFlag() == true {
        defmt::info!("Flag Found");
        if input.readFSM() == false {
            input.read(&mut data[0..128]);
            defmt::info!("ShortFrame read");
            for i in 0..128 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }else {
            input.read(&mut data[0..250]);
            defmt::info!("LongFrame read");
            for i in 0..250 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }
    }

    defmt::info!("Checkmaker {}", checkmaker);
    if checkmaker == 200 {
        defmt::info!("Long frame is good");
    }
    
    if checkmaker == 128 {
        defmt::info!("Short frame is good");
    }
    
    output.LongCallsign();
    output.write(&storeThisData);

    //hvis data er klar, læs den rigtige mænge data. 
    let mut checkmaker: u8 = 0;
    if input.readFlag() == true {
        defmt::info!("Flag Found");
        if input.readFSM() == true {
            input.read(&mut data[0..128]);
            defmt::info!("ShortFrame read");
            for i in 0..128 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }else {
            input.read(&mut data[0..250]);
            defmt::info!("LongFrame read");
            for i in 0..250 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }
    }

    defmt::info!("Checkmaker {}", checkmaker);
    if checkmaker == 200 {
        defmt::info!("Long frame is good");
    }
    
    if checkmaker == 128 {
        defmt::info!("Short frame is good");
    }
   
    output.LongCallsign();
    output.write(&storeThisData);

    //hvis data er klar, læs den rigtige mænge data. 
    let mut checkmaker: u8 = 0;
    if input.readFlag() == true {
        defmt::info!("Flag Found");
        if input.readFSM() == true {
            input.read(&mut data[0..128]);
            defmt::info!("ShortFrame read");
            for i in 0..128 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }else {
            input.read(&mut data[0..250]);
            defmt::info!("LongFrame read");
            for i in 0..250 {
                defmt::info!("Datain: {:x}, dataexpected: {:x}", data[i], storeThisData[i]);
                if data[i] == storeThisData[i]{
                    defmt::info!("I: {} ",i);
                    checkmaker = checkmaker + 1;
                }
            }
        }
    }

    defmt::info!("Checkmaker {}", checkmaker);
    if checkmaker == 200 {
        defmt::info!("Long frame is good");
    }
    
    if checkmaker == 128 {
        defmt::info!("Short frame is good");
    }

    //output.LongCallsign();
    //output.write(&storeThisData);

    loop {
    }

//This part is an example on how to externalize an arbitraty spi controller
pub mod spi_external {
    //Bunch of includes to make typedefinition easier.
    use cortex_m::prelude::_embedded_hal_blocking_spi_Write;
    use stm32f4xx_hal::{spi::{Instance, Spi}, gpio, gpio::Pin};
    use embedded_hal::{digital::v2::PinState, spi::FullDuplex};
    //Spi struct
    pub struct HalSpi<SPI: Instance, PINS,   const P: char, const N: u8, MODE> {
        spi: Spi<SPI, PINS>, //Our Hal spi
        cs: Pin<P, N, gpio::Output<MODE>>, //Chip select pin
        cs_active: PinState //Active state
    }

    //All functions
    impl <SPI: Instance, PINS, const P: char, const N: u8, MODE>
    HalSpi<SPI, PINS, P, N, MODE> {
        //Constructor
        pub fn new(spi: Spi<SPI, PINS>, cs: gpio::Pin<P, N, gpio::Output<MODE>>, cs_active: PinState) -> Self {
            HalSpi {
                spi,
                cs,
                cs_active,
            }
        }
        //Set chip select inactive.
        pub fn init(&mut self) {
            if self.cs.get_state() == self.cs_active {
                self.cs.toggle();
            }
        }
        //Write only function. Toggles CS
        pub fn write(&mut self, data: &[u8]) {
            self.cs.set_state(self.cs_active);
            self.spi.write(&data).unwrap();
            self.cs.toggle();
        }
        //Internal read function
        fn read(&mut self, cnt: u8) {
            for i in 0..cnt { //Sends 20 trailing zeros.
                while self.spi.is_busy() {} //We are waiting for last transmission to finish.
                self.spi.send(0x0).unwrap(); //.send is not blocking incoming data.
                while self.spi.is_rx_not_empty() { //Checks the RXNE flag.
                    match self.spi.read() { 
                        Ok(w) => defmt::info!("In: {:x}",  w), //Prints incoming value.
                        //NOTE this slows the communication ALOT! Save then printmonitor for actual use.
                        Err(_err) => continue,
                    };
                } 
            }
        }
        //Test function for reading the flash id.
        pub fn manid(&mut self) {
            self.cs.set_state(self.cs_active); //Set CS active
            self.spi.write(&[0x90, 0,0,0]).unwrap(); //Write with no read
            self.read(2); //Reads and displays two bytes.
            self.cs.toggle(); //Set CS inactive.
        }
    }
}
}
