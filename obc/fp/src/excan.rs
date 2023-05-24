pub mod excan {
    use bxcan::{ExtendedId, Frame};
    /*TODO: Make an actual externalization of CAN. It would be easier if all CAN handling happend from here.*/

    pub struct IdentifierContents {
        /*IdentifierContents holds the nessesary data from CAN. */
        pub prio: u8,        //priority of the message
        pub rec: u8,         //reciever ID
        pub port: u8,        //Reciever port
        pub cmd: u8,         //Command recieved
        pub trans: u8,       //Transmitter ID
        pub start_bit: bool, //true if start bit is set - First frame in a message
        pub end_bit: bool,   //true if end bit is set - Last frame in a message
        pub frg_count: u8, //Number of fragments in a message - increments for each frame in a message
    }
    impl IdentifierContents {
        pub fn frame_splitter(frame: &bxcan::Frame) -> IdentifierContents {
            /*Frame_splitter is used to extract the contents of a bxcan::frame, and store them in IdentifierContents struct.
            Supports both Extended and Standard frame, but only extended makes sense in this application.
            Standard frames should either be discarded, throw an error, or get some added functionality.
            Match statement compares data type against standard and extended. Extracts identifier, and stores*/
            match frame.id() {
                bxcan::Id::Standard(identifier) => {
                    //defmt::debug!("Standard frame detected");
                    //Hiver standard identifieren ud, og gemmer den som u32
                    let raw_id = identifier.as_raw() as u32;
                    //defmt::debug!("ID-hex is: {} / {:#05X} / {:#013b}", raw_id, raw_id, raw_id);
                    let f_id = IdentifierContents {
                        //Isoler de bits der er af interesse, og skub dem til lsb
                        prio: ((raw_id >> 26) & 0b00000111) as u8,
                        rec: ((raw_id >> 22) & 0b00001111) as u8,
                        port: ((raw_id >> 19) & 0b00000111) as u8,
                        cmd: ((raw_id >> 11) & 0b11111111) as u8,
                        trans: ((raw_id >> 7) & 0b00001111) as u8,
                        start_bit: ((raw_id >> 6) == 0b00000001),
                        end_bit: ((raw_id >> 5) == 0b00000001),
                        frg_count: (raw_id as u8 & 0b00011111),
                    };
                    //defmt::debug!("rec: {}, port: {}, cmd: {}", f_id.rec, f_id.port, f_id.cmd);
                    f_id //Returns an IdentifierContents
                }
                bxcan::Id::Extended(identifier) => {
                    //defmt::debug!("Extended frame detected");
                    let raw_id = identifier.as_raw();
                    //Hiver extended identifieren ud, og gemmer den som u32
                    //Printer den som decimal, binær og hex
                    //defmt::debug!("ID is: {} / {:#031b} / {:#010X}", raw_id, raw_id, raw_id);
                    let f_id = IdentifierContents {
                        //extended frame
                        //Der tages udgangspunkt i at frame opbygning |xxxxx|xxxxxx|xxxxxx|xxxxxx|
                        //                                             prio.| Rec. | Port |Trans.|
                        //Isoler de bits der er af interesse, og skub dem til lsb
                        prio: ((raw_id >> 26) & 0b00000111) as u8,
                        rec: ((raw_id >> 22) & 0b00001111) as u8,
                        port: ((raw_id >> 19) & 0b00000111) as u8,
                        cmd: ((raw_id >> 11) & 0b11111111) as u8,
                        trans: ((raw_id >> 7) & 0b00001111) as u8,
                        start_bit: ((raw_id >> 6) & 1) == 1,
                        end_bit: ((raw_id >> 5) & 1) == 1,
                        frg_count: (raw_id as u8 & 0b00011111),
                    };
                    //defmt::debug!("rec: {}, port: {}, cmd: {}", f_id.rec, f_id.port, f_id.cmd);
                    f_id //Returns an IdentifierContents
                }
            }
        }

        pub fn print(&self) {
            //Debug function - prints all the data in the struct as binary and hex
            defmt::debug!("prio is {:#05b} / {:#04X}", self.prio, self.prio);
            defmt::debug!("rec is {:#04b} / {:#03X}", self.rec, self.rec);
            defmt::debug!("port is {:#04b} / {:#03X}", self.port, self.port);
            defmt::debug!("cmd is {:#06b} / {:#04X}", self.cmd, self.cmd);
            defmt::debug!("trans is {:#04b} / {:#03X}", self.trans, self.trans);
            defmt::debug!(
                "start_bit is {:#03b} / {:#03X}",
                self.start_bit,
                self.start_bit
            );
            defmt::debug!("end_bit is {:#03b} / {:#03X}", self.end_bit, self.end_bit);
            defmt::debug!(
                "frg_count is {:#07b} / {:#04X}",
                self.frg_count,
                self.frg_count
            );
        }
    }

    pub fn build_id(
        /*Taskes all the elements of a task in, and compiles it into a Frame, ready to be sent to CAN.
        @TODO: build_id should tak a &[u8] slice in instead of data, to allow for more flexible data transfer. As of now, every frame use 8 bytes.*/
        prio: u8,
        rec: u8,
        port: u8,
        cmd: u8,
        start_bit: bool,
        end_bit: bool,
        frg_count: u8,
        data: &[u8; 8],
    ) -> bxcan::Frame {
        static TRANSMITTER_ID: u8 = 1; //Should be globally defined during init
        let frame = {
            //Opsætter det korrekte frame format
            //Create a new frame with the correct ID
            //[PPPRRRRpppCCCCCCCCTTTTSEFFFFF]
            let mut id = (prio << 4) as u32;
            id = (id | (rec as u32)) << 3;
            id = (id | (port as u32)) << 8;
            id = (id | (cmd as u32)) << 4;
            id = (id | (TRANSMITTER_ID as u32)) << 1;
            id = (id | (start_bit as u32)) << 1;
            id = (id | (end_bit as u32)) << 5;
            id = id | frg_count as u32;
            Frame::new_data(ExtendedId::new(id).unwrap(), *data)
        };
        frame
    }
}
