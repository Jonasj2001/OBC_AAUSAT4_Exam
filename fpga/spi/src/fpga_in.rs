

//Bunch of includes to make typedefinition easier.
    use cortex_m::prelude::_embedded_hal_blocking_spi_Write;
    use stm32f4xx_hal::{spi::{Instance, Spi, Error}, gpio, gpio::Pin, pac::rcc::csr::CSR_SPEC};
    use embedded_hal::{digital::v2::PinState, spi::FullDuplex};
    //Spi struct
    
     
    //Take address in format: |Dummy|A23-A16|A15-A8|A7-A0|
    //Output in format [A23-16, A15-A8, A7-A0]
    pub struct FpgaIn<SPI: Instance, PINS,   const P: char, const N: u8, MODE> {
        spi: Spi<SPI, PINS>, //Our Hal spi
        cs: Pin<P, N, gpio::Output<MODE>>, //Chip select pin
    }
    impl <SPI: Instance, PINS, const P: char, const N: u8, MODE>
    FpgaIn<SPI, PINS, P, N, MODE> {
        pub fn new (spi: Spi<SPI, PINS>, cs: gpio::Pin<P, N, gpio::Output<MODE>>) -> Self {
            FpgaIn {
                spi,
                cs,
            }
        }

        pub fn write(&mut self, data: &[u8]) {
            while self.spi.is_busy() {}
            self.cs.set_low();
            self.spi.write(&data).unwrap();
            self.cs.toggle();
        }

        pub fn correct_callsign(&mut self) {
            let callsign = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
            let fsm = [0xff];
            
            self.cs.set_low();
            for i in 0..256 {
                while self.spi.is_busy() {}
                if i == 69 {
                    self.spi.write(&callsign).unwrap();
                    while self.spi.is_busy() {}
                    self.spi.write(&fsm).unwrap();
                } else {
                self.spi.write(&[i as u8]).unwrap();
                }
            }

        }
    }
