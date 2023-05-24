//Hal_02.rs - nb
fn send(&mut self, byte: W) -> nb::Result<(), Error> {
    if BIDI {
        self.spi.cr1.modify(|_, w| w.bidioe().set_bit());
    }
    self.check_send(byte)
}

//Hal_02.rs - blocking

impl<SPI, PINS, const BIDI: bool> Write<u8> for Spi<SPI, PINS, BIDI, u8>
where
    SPI: Instance,
{
    type Error = Error;

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        self.write_iter(words.iter().copied())
    }
}

impl<SPI, PINS, const BIDI: bool> WriteIter<u8> for Spi<SPI, PINS, BIDI, u8>
where
    SPI: Instance,
{
    type Error = Error;

    fn write_iter<WI>(&mut self, words: WI) -> Result<(), Self::Error>
    where
        WI: IntoIterator<Item = u8>,
    {
        for word in words.into_iter() {
            nb::block!(self.send(word))?;
            if !BIDI {
                nb::block!(self.read())?;
            }
        }

        Ok(())
    }
}

//SPI.rs
impl<SPI, PINS, const BIDI: bool, W, OPERATION> ReadWriteReg<W>
    for Spi<SPI, PINS, BIDI, W, OPERATION>
where
    SPI: Instance,
    W: FrameSize,
{
    fn read_data_reg(&mut self) -> W {
        // NOTE(read_volatile) read only 1 byte (the svd2rust API only allows
        // reading a half-word)
        unsafe { ptr::read_volatile(&self.spi.dr as *const _ as *const W) }
    }

    fn write_data_reg(&mut self, data: W) {
        // NOTE(write_volatile) see note above
        unsafe { ptr::write_volatile(&self.spi.dr as *const _ as *mut W, data) }
    }
}
