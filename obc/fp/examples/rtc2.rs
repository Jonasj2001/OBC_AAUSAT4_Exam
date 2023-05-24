#![no_main]
#![no_std]

//TODO: Implementer et struct med EXTI, PWR og RTC i - lav en "opsæt", en "reschedule" og en "detrigger" funktion :)

use stm32f446_rtic as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1,USART2])]
mod app {
    use dwt_systick_monotonic::{DwtSystick, ExtU32};
    use stm32f446_rtic::exrtc::exrtc::{self as er};
    use stm32f4xx_hal::{
        pac::{self, EXTI, PWR},
        //gpio::{gpiob::PB15},
        prelude::*,
        rtc::Rtc,
    };
    use time::{
        macros::{date, offset, time},
        PrimitiveDateTime,
    };

    // Needed for scheduling monotonic tasks
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<48_000_000>; // 48 MHz

    // Holds the shared resources (used by multiple tasks)
    // Needed even if we don't use it
    #[shared]
    struct Shared {
        rtc: er::RTCSTRUCT,
    }

    // Holds the local resources (used by a single task)
    // Needed even if we don't use it
    #[local]
    struct Local {}

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
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        let mut rtc = Rtc::new_lsi(_device.RTC, &mut _device.PWR);
        // rtc.set_datetime(date)
        // let mut delay = _device.TIM5.delay_us(&clocks);
        rtc.set_date(&date!(1970 - 01 - 01)).unwrap();
        rtc.set_time(&time!(00:00:00)).unwrap();

        // rtc.set_datetime(&PrimitiveDateTime::new(
        //     date!(2023 - 05 -02),
        //     time!(13:20)
        // )).unwrap();

        // enable tracing and the cycle counter for the monotonic timer
        _core.DCB.enable_trace();
        _core.DWT.enable_cycle_counter();

        let rtc = er::RTCSTRUCT::new(_device.EXTI, _device.PWR, rtc);
        //OPSÆTNING ALARM BEGYND
        rtc.setup(5);
        //OPSÆTNING ALARM SLUT

        // Set up the monotonic timer
        let mono = DwtSystick::new(&mut _core.DCB, _core.DWT, _core.SYST, clocks.hclk().to_Hz());
        task1::spawn_after(1.secs()).ok();
        //alarm_task::spawn_after(15.secs()).ok();
        (Shared { rtc }, Local {}, init::Monotonics(mono))
    }

    // The idle function is called when there is nothing else to do
    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    // The task functions are called by the scheduler
    #[task(shared=[rtc],)]
    fn task1(ctx: task1::Context) {
        let mut rtc_ctx = ctx.shared.rtc;
        rtc_ctx.lock(|f| f.print_time());
        task1::spawn_after(1.secs()).ok();
    }

    #[task(binds = RTC_ALARM)]
    fn trigger_task(ctx: trigger_task::Context) {
        alarm_task::spawn().ok();
    }

    #[task(shared = [rtc],priority = 2)]
    fn alarm_task(ctx: alarm_task::Context) {
        let mut rtc_ctx = ctx.shared.rtc;
        rtc_ctx.lock(|f| f.disable_alarm());
        rtc_ctx.lock(|f| f.set_alarm_time(10));
        rtc_ctx.lock(|f| f.enable_alarm());
        let imrstatus = rtc_ctx.lock(|f| f.exti.imr.read().mr17().bit());
        let rtsrstatus = rtc_ctx.lock(|f| f.exti.rtsr.read().tr17().bit());

        defmt::info!("IMR status: {}", imrstatus);
        defmt::info!("RTSR status: {}", rtsrstatus);

        let alcr = rtc_ctx.lock(|f| f.rtc.regs.cr.read().bits());
        defmt::info!("CR: {:#034b}", alcr);
        let ala = rtc_ctx.lock(|f| f.rtc.regs.alrmar().read().bits());
        defmt::info!("AlarmA: {:#034b}", ala);
        let alraf = rtc_ctx.lock(|f| f.rtc.regs.isr.read().alraf().bit());
        defmt::info!("ALRAF: {}", alraf);
        defmt::info!("TRIGGERED!");
    }
}
