pub mod excan {
    use bxcan::{filter::Mask32,Fifo,ExtendedId, Frame, Instance};
    //En skabelon for opbygningen af en identifier
    use stm32f4xx_hal::{
        can::Can,
        gpio::{
            gpioa::{PA11, PA12, PA5},
            Alternate, Output, PushPull,
        },
        pac::CAN1,
        prelude::*,
    };
    
    pub struct IdentifierContents {
        /*Et forsøg på at lave en opbygning i stil med CSP.
        Idéen er at forskellige funktioner i et modul tilgås vha porten. */
        pub prio: u8, //priority of the message
        pub rec: u8,  //reciever ID
        pub port: u8, //Reciever port
        pub cmd: u8,
        pub trans: u8, //Transmitter ID
        pub frg: u8,   //flags undefined - men med plads til dem :)
        pub frg_count: u8,
        pub std_frame: bool, //Er det standard eller extended?
    }
    impl IdentifierContents {
        pub fn frame_splitter(frame: &bxcan::Frame) -> IdentifierContents {
            /*Frame_splitter kan benyttes til at hente en bxcan::frame sendt direkte fra CAN,
            og herefter splitte den op i kategorier. Her benyttes bit operationer til at isolere de relevante data.
            Understøtter både Extended og standard frame.*/
            //Matcher typen af frame.id() mod std_frame og extended for at finde hvilken type der benyttes - rå data udtrækkes
            match frame.id() {
                bxcan::Id::Standard(identifier) => {
                    defmt::info!("Standard frame detected");
                    //Hiver standard identifieren ud, og gemmer den som u32
                    let raw_id = identifier.as_raw() as u32;
                    defmt::info!("ID-hex is: {} / {:#05X} / {:#013b}", raw_id, raw_id, raw_id);
                    let f_id = IdentifierContents {
                        //Isoler de bits der er af interesse, og skub dem til lsb
                        prio: ((raw_id >> 8) & 0b00000111) as u8,
                        rec: ((raw_id >> 5) & 0b00000111) as u8,
                        port: ((raw_id >> 3) & 0b00000011) as u8,
                        cmd: 0,
                        trans: (raw_id & 0b00000111) as u8,
                        frg: 0,
                        frg_count: 0,
                        std_frame: true,
                    };
                    f_id //Returnerer en IdentifierContents
                }
                bxcan::Id::Extended(identifier) => {
                    defmt::info!("Extended frame detected");
                    let raw_id = identifier.as_raw();
                    //Hiver extended identifieren ud, og gemmer den som u32
                    //Printer den som decimal, binær og hex
                    defmt::info!("ID is: {} / {:#031b} / {:#010X}", raw_id, raw_id, raw_id);
                    let f_id = IdentifierContents {
                        //extended frame
                        //Der tages udgangspunkt i at frame opbygning |xxxxx|xxxxxx|xxxxxx|xxxxxx|
                        //                                             prio.| Rec. | Port |Trans.|
                        //Isoler de bits der er af interesse, og skub dem til lsb
                        prio: ((raw_id >> 24) & 0b00011111) as u8,
                        rec: ((raw_id >> 19) & 0b00001111) as u8,
                        port: ((raw_id >> 16) & 0b00001111) as u8,
                        cmd: ((raw_id >> 10) & 0b00111111) as u8,
                        trans: ((raw_id >> 6) & 0b00001111) as u8,
                        frg: ((raw_id >> 3) & 0b00000111) as u8,
                        frg_count: ((raw_id) & 0b00000111) as u8,
                        std_frame: false,
                    };
                    f_id //Returnerer en IdentifierContents
                }
            }
        }

        pub fn print(&self) {
            //Benyttes til at printe funktionen - printer alting som deci og hex
            if self.std_frame {
                defmt::info!("prio is {:#05b} / {:#04X}", self.prio, self.prio);
                defmt::info!("rec is {:#04b} / {:#03X}", self.rec, self.rec);
                defmt::info!("port is {:#04b} / {:#03X}", self.port, self.port);
                defmt::info!("cmd is {:#06b} / {:#04X}", self.cmd, self.cmd);
                defmt::info!("trans is {:#04b} / {:#03X}", self.trans, self.trans);
                defmt::info!("trans is {:#03b} / {:#03X}", self.trans, self.trans);
                defmt::info!("trans is {:#03b} / {:#03X}", self.trans, self.trans);
            } else {
                defmt::info!("prio is {:#05b} / {:#04X}", self.prio, self.prio);
                defmt::info!("rec is {:#04b} / {:#03X}", self.rec, self.rec);
                defmt::info!("port is {:#04b} / {:#03X}", self.port, self.port);
                defmt::info!("cmd is {:#06b} / {:#04X}", self.cmd, self.cmd);
                defmt::info!("trans is {:#04b} / {:#03X}", self.trans, self.trans);
                defmt::info!("frg is {:#03b} / {:#03X}", self.frg, self.frg);
                defmt::info!(
                    "frg_count is {:#03b} / {:#03X}",
                    self.frg_count,
                    self.frg_count
                );
                defmt::info!("std is {}", self.std_frame);
            }
        }
    }

    pub fn build_id(
        /*TODO: Byg data pakken selv, baseret på typen der kommer ind.
        TODO: Implementer fragment counter*/
        prio: u8,
        rec: u8,
        port: u8,
        cmd: u8,
        frg: u8,
        frg_count: u8,
        data: &[u8; 8],
    ) -> bxcan::Frame {
        defmt::debug!(
            "Build content: prio={}, rec={}, port={},cmd={}",
            prio,
            rec,
            port,
            cmd,
        );
        //Måske indfør en funktion der tjekker at tallene er indenfor den angivne længde?
        static TRANSMITTER_ID: u8 = 11; //Definerer hvilken ID boardet skal markeres som
                                        //GET NUMBER OF FRAGMENTS - LOOP AND SIZE???
        let frame = {
            //Opsætter det korrekte frame format
            let mut id = (prio << 5) as u32;
            defmt::debug!("ID step 1: {:#031b}", id);
            id = (id | (rec as u32)) << 4;
            defmt::debug!("ID step 2: {:#031b}", id);
            id = (id | (port as u32)) << 4;
            defmt::debug!("ID step 3: {:#031b}", id);
            id = (id | (cmd as u32)) << 6;
            defmt::debug!("ID step 4: {:#031b}", id);
            id = (id | (TRANSMITTER_ID as u32)) << 4;
            defmt::debug!("ID step 5: {:#031b}", id);
            id = (id | (frg as u32)) << 3;
            defmt::debug!("ID step 6: {:#031b}", id);
            id = (id | (frg_count as u32)) << 3;
            defmt::debug!("ID step 7: {:#031b}", id);
            Frame::new_data(ExtendedId::new(id).unwrap(), *data)
        };
        defmt::info!("Transmitted frame: {:#06X}", frame);
        frame
    }
}
