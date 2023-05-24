#![no_std]

//Bunch of includes to make typedefinition easier.
    use cortex_m::prelude::_embedded_hal_blocking_spi_Write;
    use stm32f4xx_hal::{spi::{Instance, Spi, Error}, gpio, gpio::{Pin, Input, PinMode}, pac::rcc::csr::CSR_SPEC};
    use embedded_hal::{digital::v2::PinState, spi::FullDuplex};
    //Spi struct
    
     
    //Take address in format: |Dummy|A23-A16|A15-A8|A7-A0|
    //Output in format [A23-16, A15-A8, A7-A0]
    pub struct FpgaOut<SPI: Instance, PINS,   const P: char, const N: u8, const Y: char, const K: u8> {
        spi: Spi<SPI, PINS>, //Our Hal spi
        flag: Pin<P, N>,
        fsm: Pin<Y,K>,
    }
    impl <SPI: Instance, PINS, const P: char, const N: u8, const Y: char, const K: u8>
    FpgaOut<SPI, PINS, P, N, Y, K> {
         pub fn new (spi: Spi<SPI, PINS>, flag: Pin<P, N>, fsm: Pin<Y,K>) -> Self {
             FpgaOut {
                 spi,
                 flag,
                 fsm
             }
         }
         pub fn readFlag(&mut self) -> bool {
             self.flag.is_high()
         }

         pub fn readFSM(&mut self) -> bool{
            self.fsm.is_high()
         }


         pub fn read(&mut self, data: &mut[u8]) {
            for i in 0..data.len() {
                while self.spi.is_busy() {}
                self.spi.send(0x0).unwrap();
                while self.spi.is_busy() {}
                data[i] = self.spi.read().unwrap();
        }
    }
}
