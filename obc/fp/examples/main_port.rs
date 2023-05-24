#![no_main]
#![no_std]

//use stm32f446_rtic as _; // global logger + panicking-behavior + memory layout
use rtic_playtime::{self as _}; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1, USART2])]
mod app {

    use bxcan::filter::Mask32;
    use bxcan::{Data, Fifo};
    use dwt_systick_monotonic::{DwtSystick, ExtU32};
    use rtic_playtime::excan::deepcan::{self as dc};
    use stm32f4xx_hal::{
        can::Can,
        gpio::{
            gpioa::{PA11, PA12, PA5},
            Alternate, Output, PushPull,
        },
        pac::CAN1,
        prelude::*,
    };
    // Needed for scheduling monotonic tasks
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<180_000_000>; // 180 MHz

    // Holds the shared resources (used by multiple tasks)
    // Needed even if we don't use it
    #[shared]
    struct Shared {
        //can1 opsættes til den interne CAN1, og bliver linket til PA12 og PA11 på den alternative funktion 9.
        can1: bxcan::Can<Can<CAN1, (PA12<Alternate<9>>, PA11<Alternate<9>>)>>,
    }

    // Holds the local resources (used by a single task)
    // Needed even if we don't use it
    #[local]
    struct Local {
        led: PA5<Output<PushPull>>,
    }

    // The init function is called in the beginning of the program
    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");
        // Cortex-M peripherals
        let mut _core: cortex_m::Peripherals = ctx.core;

        // Device specific peripherals
        let mut _device: stm32f4xx_hal::pac::Peripherals = ctx.device;

        // Set up the system clock.
        let rcc = _device.RCC.constrain();
        //let clocks = rcc.cfgr.sysclk(180.MHz()).freeze(); // Important: 45 MHz is the max for CAN since it has to match the APB1 clock
        let clocks = rcc.cfgr.sysclk(180.MHz()).pclk1(45.MHz()).freeze(); //Eksperiment :)

        defmt::debug!("AHB1 clock: {} Hz", clocks.hclk().to_Hz());
        defmt::debug!("APB1 clock: {} Hz", clocks.pclk1().to_Hz());

        // Set up the LED. On the Nucleo-F446RE it's connected to pin PA5.
        let gpioa = _device.GPIOA.split();
        let led = gpioa.pa5.into_push_pull_output();

        // Initialize variables for can_send

        // Set up CAN device 1
        let mut can1 = {
            // CAN pins alternate function 9 as per datasheet
            // https://www.st.com/resource/en/datasheet/stm32f446mc.pdf page 57
            let rx = gpioa.pa11.into_alternate::<9>();
            let tx = gpioa.pa12.into_alternate::<9>();

            // let can = Can::new(dp.CAN1, (tx, rx));
            // or
            let can = _device.CAN1.can((tx, rx));

            defmt::info!("CAN1, waiting for 11 recessive bits...");
            bxcan::Can::builder(can)
                // APB1 (PCLK1): 45MHz, Bit rate: 1MBit/s, Sample Point 87.5%
                // Value was calculated with http://www.bittiming.can-wiki.info/
                .set_bit_timing(0x001b0002)
                .set_automatic_retransmit(true)
                // .set_silent(true)
                .enable()
        };

        defmt::info!("CAN1, waiting for 11 recessive bits... (done)");

        can1.enable_interrupts({
            use bxcan::Interrupts as If;
            If::FIFO0_MESSAGE_PENDING | If::FIFO0_FULL | If::FIFO0_OVERRUN
        });

        // Configure filters so that can frames can be received.
        can1.modify_filters()
            .enable_bank(0, Fifo::Fifo0, Mask32::accept_all());

        // enable tracing and the cycle counter for the monotonic timer
        _core.DCB.enable_trace();
        _core.DWT.enable_cycle_counter();

        // Set up the monotonic timer
        let mono = DwtSystick::new(&mut _core.DCB, _core.DWT, _core.SYST, clocks.hclk().to_Hz());

        defmt::info!("Init done!");
        blink::spawn_after(1.secs()).ok();
        can_send::spawn(
            0,
            0,
            0,
            0,
            false,
            bxcan::Data::from([0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF]),
        )
        .ok();
        (Shared { can1 }, Local { led }, init::Monotonics(mono))
    }

    // The idle function is called when there is nothing else to do
    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    // The task functions are called by the scheduler
    #[task(local = [led])]
    fn blink(ctx: blink::Context) {
        ctx.local.led.toggle();
        defmt::debug!("Blink!");
        blink::spawn_after(5.secs()).ok();
    }

    // send a meesage via CAN
    #[task(shared = [can1], priority=2)]
    fn can_send(
        mut ctx: can_send::Context,
        priority: u8,
        reciever: u8,
        port: u8,
        flags: u8,
        standard_frame: bool,
        data: Data,
    ) {
        //@TODO: Fix this
        let frame = dc::build_id(priority, reciever, port, flags, standard_frame, data);
        defmt::info!("Sending frame with first byte: {:#06X}", data);
        ctx.shared.can1.lock(|can1| can1.transmit(&frame).unwrap());
    }

    // receive a message via CAN
    #[task(binds = CAN1_RX0, shared = [can1])]
    fn can_receive(ctx: can_receive::Context) {
        let mut can1 = ctx.shared.can1;
        let frame = can1.lock(|can1| can1.receive().unwrap());
        defmt::info!("Complete frame: {}", frame);

        //Initialiserer frame_id for som struct, for at få adgang til funktionen
        let frame_id = dc::IdentifierContents::frame_splitter(&frame);
        //frame_id = frame_id.frame_splitter(frame);
        frame_id.print();

        let frame_data = frame.data().unwrap();
        let data: [u8; 8] = [0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF];
        let framedata = bxcan::Data::from(data);

        match frame_id.port {
            //Port 0: Repeat / response
            0 => can_send::spawn(
                frame_id.port,
                frame_id.trans,
                0,
                0,
                frame_id.std_frame,
                *frame_data,
            )
            .ok(),
            //Port 1: Event log
            1 => {
                write_to_event_log::spawn(frame_id.trans, *frame_data).ok();
                can_send::spawn(0, 0, 0, 0, false, framedata).ok()
            }
            //Port 2: Error log
            2 => {
                write_to_error_log::spawn(frame_id.trans, *frame_data, frame_id.flags).ok();
                can_send::spawn(0, 0, 0, 0, false, framedata).ok()
            }
            //Port 0 & 3-255: Ikke implementeret.
            3_u8..=u8::MAX => can_send::spawn(0, 0, 0, 0, false, framedata).ok(),
        };
        //Send vilkårlig frame
    }

    #[task()] //Kode stub til at skrive til en event log
    fn write_to_event_log(_ctx: write_to_event_log::Context, id: u8, data: bxcan::Data) {
        let time = "RTC-VALUE";
        defmt::debug!(
            "To the event log: Time: {}, Module: {:#08b}, Data: {:#04X}",
            time,
            id,
            data
        );
    }

    #[task()] //Kodestub til at skrive til en error log
    fn write_to_error_log(_ctx: write_to_error_log::Context, id: u8, data: bxcan::Data, flags: u8) {
        let time = "RTC-VALUE";
        defmt::debug!(
            "To the error log: Time: {}, Module: {:#08b}, flags: {:#08b}, Data: {:#08X}",
            time,
            id,
            flags,
            data
        );
    }
}
