pub mod exrtc {
    use stm32f4xx_hal::{
        pac::{EXTI, PWR},
        rtc::Rtc,
    };
    use time::{macros::offset, OffsetDateTime};
    pub struct RTCSTRUCT {
        pub exti: EXTI,
        pub pwr: PWR,
        pub rtc: Rtc<stm32f4xx_hal::rtc::Lsi>,
    }

    impl RTCSTRUCT {
        pub fn new(
            exti: EXTI,
            pwr: PWR,
            rtc: Rtc<stm32f4xx_hal::rtc::Lsi>,
            start_time: i32,
        ) -> Self {
            let rtc = RTCSTRUCT {
                exti: exti,
                pwr: pwr,
                rtc: rtc,
            };
            //Setup Interrupt route
            rtc.exti.imr.write(|f| f.mr17().set_bit());
            rtc.exti.rtsr.write(|f| f.tr17().set_bit());

            //Disable write protection
            rtc.pwr.cr.write(|f| f.dbp().set_bit());
            rtc.rtc.regs.wpr.write(|f| f.key().bits(0xCA));
            rtc.rtc.regs.wpr.write(|f| f.key().bits(0x53));

            rtc.set_alarm_time(start_time);
            rtc.print_alarm_and_cr();

            rtc
        }

        pub fn disable_alarm(&self) {
            self.disable_alarm_internal();
            self.write_disable()
        }

        fn disable_alarm_internal(&self) {
            self.write_enable();
            //Clear flag og enable
            self.rtc.regs.isr.write(|f| f.alraf().clear_bit());
            self.rtc.regs.cr.write(|f| f.alrae().clear_bit());

            //"This bit is set when the selected edge event arrives on the external interrupt line.
            //This bit is cleared by programming it to ‘1’"
            self.exti.pr.write(|f| f.pr17().set_bit());
            self.exti.pr.write(|f| f.pr17().clear_bit());
        }

        pub fn set_alarm_time(&self, time: i32) {
            self.disable_alarm_internal();
            //alrawf er høj når der kan skrives til registret
            while self.rtc.regs.isr.read().alrawf().bit_is_clear() {
                continue;
            }

            //Tager en unix funktion, og returnerer som DD:HH:MM:SS format (t:tens = tiere, u: units = en'ere)
            let (dt, du, ht, hu, mnt, mnu, st, su) = self.transform_time(time);
            //Sætter næste alarmtidspunkt i registret
            self.rtc.regs.alrmar().write(|f| {
                f.dt()
                    .bits(dt)
                    .du()
                    .bits(du)
                    .ht()
                    .bits(ht)
                    .hu()
                    .bits(hu)
                    .mnt()
                    .bits(mnt)
                    .mnu()
                    .bits(mnu)
                    .st()
                    .bits(st)
                    .su()
                    .bits(su)
                    //Sætter triggers - Trigger on Day : Hour : Minute : second
                    .msk1()
                    .clear_bit()
                    .msk2()
                    .clear_bit()
                    .msk3()
                    .clear_bit()
                    .msk4()
                    .clear_bit()
            });
            self.enable_alarm();
        }

        fn enable_alarm(&self) {
            //Setup Interrupt route
            self.exti.imr.write(|f| f.mr17().set_bit());
            self.exti.rtsr.write(|f| f.tr17().set_bit());
            //Aktiverer Interrup, routing, polariteten og enable igen
            self.rtc.regs.cr.write(|f| {
                f.alraie()
                    .set_bit()
                    //.osel()
                    //.alarm_a()
                    //.pol()
                    //.high()
                    .alrae()
                    .set_bit()
            });
            self.write_disable();
            //self.print_alarm_and_cr();
        }

        fn transform_time(&self, unixtime: i32) -> (u8, u8, u8, u8, u8, u8, u8, u8) {
            let time = OffsetDateTime::from_unix_timestamp(unixtime as i64).unwrap();
            let (_, _, d) = time.to_calendar_date();
            let (h, m, s) = time.to_hms();
            let dt: u8 = d / 10;
            let du: u8 = d % 10;
            let ht: u8 = h / 10;
            let hu: u8 = h % 10;
            let mnt: u8 = m / 10;
            let mnu: u8 = m % 10;
            let st: u8 = s / 10;
            let su: u8 = s % 10;
            (dt, du, ht, hu, mnt, mnu, st, su)
        }

        fn write_enable(&self) {
            self.pwr.cr.write(|f| f.dbp().set_bit());
            self.rtc.regs.wpr.write(|f| f.key().bits(0xCA));
            self.rtc.regs.wpr.write(|f| f.key().bits(0x53));
        }

        fn write_disable(&self) {
            self.rtc.regs.wpr.write(|f| f.key().bits(0));
            self.pwr.cr.write(|f| f.dbp().clear_bit());
        }

        pub fn get_time(&mut self, print: bool) -> i64 {
            let time = self.rtc.get_datetime().assume_offset(offset!(UTC));
            let (_, _, d) = time.to_calendar_date();
            let (h, m, s) = time.to_hms();
            if print {
                //defmt::debug!("RTC: d: {}, h: {}, m: {}, s: {}", d, h, m, s);
            }
            time.unix_timestamp()
        }

        fn print_alarm_and_cr(&self) {
            let alrmar = self.rtc.regs.alrmar().read().bits();
            let cr = self.rtc.regs.cr.read().bits();

            //defmt::debug!("AlarmA: {:#034b}", alrmar);
            //defmt::debug!("RTC_CR: {:#026b}", cr);
        }
    }
}
