

//Bunch of includes to make typedefinition easier.
    use cortex_m::prelude::_embedded_hal_blocking_spi_Write;
    use stm32f4xx_hal::{spi::{Instance, Spi, Error}, gpio, gpio::Pin, pac::rcc::csr::CSR_SPEC};
    use embedded_hal::{digital::v2::PinState, spi::FullDuplex};
    //Spi struct
    
    const DUMMY: u8 = 0x0;
    //Standard SPI Instructions
    #[allow(unused)]
    #[repr(u8)]
    enum OpCode {
        //No trailing
        WriteEnable = 0x06,
        WriteDisable = 0x04,
        //3 byte address and data read/write
        PageProgram = 0x02,
        Read = 0x03,

        //Erase: - 3 Byte trailing address
        SectorErase = 0x20,
        BlockErase32 = 0x52,
        BlockErase64 = 0xd8,
        ChipErase1 = 0xc7, //Part 1 
        ChipErase2 = 0x60, //Part 2 

        //Registers - 1 trailing byte Read/Write
        ReadStatus1 = 0x05,
        ReadStatus2 = 0x35,
        ReadStatus3 = 0x15,

        WriteStatus1 = 0x01,
        WriteStatus2 = 0x31,
        WriteStatus3 = 0x11,


        //Ids:
        ManId = 0x90, //2 dummy and 0x0 - 2 Byte Read
        JedecId = 0x9f, //3 Byte Read
        DeviceId = 0xAB, //Three dummy, - 1 Byte Read
        UniqueId = 0x4b, //Four dummy, - 8 Byte Read

    }
    pub enum Delete {
        SectorErase,
        BlockErase32,
        BlockErase64,
        ChipErase
    }
    pub struct FlashInfo {
        pub page_size: u16,
        pub sector_size: u32,
        pub page_count: u32,
        pub sector_count: u32,
        pub block_size: u32,
        pub block_count: u32,
        pub capacity_mbit: u32,   
    }
    //Predefined flash:
    const W25Q128: FlashInfo = FlashInfo {
        page_size: 256,
        sector_size: 0x1000,
        page_count: (128 * 16 * 0x1000) / 256,
        sector_count: 128 * 16,
        block_size: 0x1000 * 16,
        block_count: 128,
        capacity_mbit: 128,
    };

    
    //Take address in format: |Dummy|A23-A16|A15-A8|A7-A0|
    //Output in format [A23-16, A15-A8, A7-A0]
    pub fn split_address (address: u32) -> [u8;3] {
        let tmp: [u8; 4] = address.to_be_bytes();
        tmp[1..].try_into().unwrap()
    }
    pub struct Memory<SPI: Instance, PINS,   const P: char, const N: u8, MODE> {
        spi: Spi<SPI, PINS>, //Our Hal spi
        cs: Pin<P, N, gpio::Output<MODE>>, //Chip select pin
        cs_active: PinState, //Active state
        flash: FlashInfo
    }
    impl <SPI: Instance, PINS, const P: char, const N: u8, MODE>
    Memory<SPI, PINS, P, N, MODE> {
        //Constructor with cusom flash parameters.
        pub fn new(spi: Spi<SPI, PINS>, cs: gpio::Pin<P, N, gpio::Output<MODE>>, flash: FlashInfo) -> Self {
            
            let mut spi = Memory {
                spi,
                cs,
                cs_active: PinState::Low,
                flash,
            };
            //Set the chipselect
            spi.init();
            spi //Return
        }
        //Constructor for the ws25j128 type.
        pub fn new_w25q128(spi: Spi<SPI, PINS>, cs: gpio::Pin<P, N, gpio::Output<MODE>>) -> Self {
            
            let mut spi = Memory {
                spi,
                cs,
                cs_active: PinState::Low,
                flash: W25Q128, //Predefine flash
            };
            spi.init(); //Init chip select 
            spi
        }
        //Change the active state of the flash:
        pub fn change_active(&mut self, state: PinState) {
            self.cs_active = state;
            self.init();
        } 
        //Set chip select inactive
        fn init(&mut self) {
            if self.cs.get_state() == self.cs_active {
                self.cs.toggle();
            }
        }
        //Return memory info:
        pub fn get_info_sectorsize(&self) -> u32 {
            self.flash.sector_size
        } 

        //Check busy bit of the flash status register (SR):
        pub fn is_busy(&mut self) -> bool {
            (self.read_status_reg() & 0b1) > 0
        }

        //Read flash SR1
        fn read_status_reg(&mut self) -> u8 {
            self.cs.set_state(self.cs_active);
            self.spi.write(&[OpCode::ReadStatus1 as u8]).unwrap_or_default();
            let status = self.read_single();
            while self.spi.is_busy() {}
            self.cs.toggle();
            status
        }

        //Read and return one byte from the SPI bus.
        //No change of the CS pin.
        fn read_single(&mut self) -> u8 {
            while self.spi.is_busy() {}
            self.spi.send(DUMMY).unwrap_or_default();
            while self.spi.is_busy() {}
            self.spi.read().unwrap()
            
        }
        //Read a predefined lenght into a buffer reference
        pub fn read(&mut self, addr: u32, len: usize, data: &mut [u8]) {
            let addr_data = split_address(addr);
            let mut readlenght = 0;

            //Checks for buffer potential bufferoverflow:
            if len > data.len() {
                readlenght = data.len(); //Set read cap at buffersize.
            } else {
                readlenght = len;
            }

            while self.is_busy() {}
            self.cs.set_state(self.cs_active);
            //Read instruction set
            self.spi.write(&[OpCode::Read as u8]).unwrap_or_default();
            self.spi.write(&addr_data).unwrap_or_default();
            
            for i in 0..readlenght {
                while self.spi.is_busy() {}
                self.spi.send(DUMMY).unwrap_or_default();
                while self.spi.is_busy() {}
                data[i] = self.spi.read().unwrap_or_default();
            }
            while self.spi.is_busy() {}
            self.cs.toggle(); //Done
        }

        //Delete functions:
        //Update to single function taking opcode: //
        fn sector_erase(&mut self, addr: [u8;3]) {
            self.write_enable();
            while self.is_busy(){}
            self.cs.set_state(self.cs_active);
            while self.spi.is_busy() {}
            self.spi.write(&[OpCode::SectorErase as u8]).unwrap();
            self.spi.write(&addr).unwrap();
            while self.spi.is_busy() {}
            self.cs.toggle();

        }
        //32kB block erase
        fn block32_erase(&mut self, addr: [u8;3]) {
            self.write_enable();
            while self.is_busy(){}
            self.cs.set_state(self.cs_active);
            while self.spi.is_busy() {}
            self.spi.write(&[OpCode::BlockErase32 as u8]).unwrap();
            self.spi.write(&addr).unwrap();
            while self.spi.is_busy() {}
            self.cs.toggle();

        }
        //64kB block erase 
         fn block64_erase(&mut self, addr: [u8;3]) {
            self.write_enable();
            while self.is_busy(){}
            self.cs.set_state(self.cs_active);
            while self.spi.is_busy() {}
            self.spi.write(&[OpCode::BlockErase64 as u8]).unwrap();
            self.spi.write(&addr).unwrap();
            while self.spi.is_busy() {}
            self.cs.toggle();

        }
        //Chip erase USE WITH CAUTION
         fn chip_erase(&mut self) {
            self.write_enable();
            while self.is_busy(){}
            self.cs.set_state(self.cs_active);
            while self.spi.is_busy() {}
            let opcode: [u8;2] = [OpCode::ChipErase1 as u8, OpCode::ChipErase2 as u8];
            self.spi.write(&opcode).unwrap();

            while self.spi.is_busy() {}
            self.cs.toggle();

        }
        //Public interface for delete functions:
        pub fn delete(&mut self, option: Delete, addr: u32) {
            let addr_split = split_address(addr);
            match option {
                Delete::SectorErase => self.sector_erase(addr_split),
                Delete::BlockErase32 => self.block32_erase(addr_split),
                Delete::BlockErase64 => self.block64_erase(addr_split),
                Delete::ChipErase => self.chip_erase(),
            }
        }

        //For single word instructions
        fn write_single(&mut self, byte: u8){
            self.cs.set_state(self.cs_active);
            while self.spi.is_busy() {}
            self.spi.write(&[byte]).unwrap_or_default();
            while self.spi.is_busy() {}
            self.cs.toggle();
        }
        //Software write enable (WEL).
        //Used for pageprogram, and erasure.
        fn write_enable(&mut self) {
            while self.is_busy() {}
            self.write_single(OpCode::WriteEnable as u8);
        }
        //Disable software WEL
        fn write_disable(&mut self) {
            self.write_single(OpCode::WriteDisable as u8);
        }

        //Programming a page in flash:
        fn write_page(&mut self, addr: [u8;3], data: &[u8]) {
            // defmt::info!("Writing {} bytes to addr: {:x}{:x}{:x}", data.len(), addr[0],addr[1], addr[2]);
            while self.is_busy() {}
            self.write_enable();
            self.cs.set_state(self.cs_active);
            self.spi.write(&[OpCode::PageProgram as u8]).unwrap_or_default();
            self.spi.write(&addr).unwrap_or_default();
            self.spi.write(data).unwrap();
            while self.spi.is_busy() {}
            self.cs.toggle();
        }
        //public Write function, allow for single aswell as multi page programming:
        pub fn write(&mut self, addr: u32, data: &[u8] ) {
            let mut address = addr; //Copy of the address
            let first = address & 0xff; //Index on page
            //Check if everything fits on the remaining space of the page:
            if (first as usize + data.len()) > self.flash.page_size as usize { 
                let mut index = 0; //Data index
                let first_page = self.flash.page_size - (first as u16); //Find data boundaries for first page

                let full_pages = (data.len() - first_page as usize) / self.flash.page_size as usize;
                //Program first page
                self.write_page(split_address(address), &data[index..first_page as usize]);
                index = first_page as usize;
                address -= first; //Set page index to 0

                //Program full pages:
                if full_pages > 0 { 
                    for i in 0..full_pages {
                        address +=0x100; //Address jump one page.
                        self.write_page(split_address(address), &data[index..(index+self.flash.page_size as usize)]);
                        index +=self.flash.page_size as usize; //Update index
                    }
                }
                address +=0x100;
                //Write last partial page.
                self.write_page(split_address(address), &data[index..]); 
            }
            //Fits on the page, write.
            else {
                self.write_page(split_address(address), data);
            }
            

        }
    }