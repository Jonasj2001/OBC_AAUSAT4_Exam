#![no_main]
#![no_std]

use stm32f446_rtic as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1])]
mod app {
    use dwt_systick_monotonic::{DwtSystick, ExtU32};
    use stm32f4xx_hal::{
        pac,
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
    struct Shared {}

    // Holds the local resources (used by a single task)
    // Needed even if we don't use it
    #[local]
    struct Local {
        rtc: Rtc<stm32f4xx_hal::rtc::Lsi>,
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
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        //
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

        // Set up the monotonic timer
        let mono = DwtSystick::new(&mut _core.DCB, _core.DWT, _core.SYST, clocks.hclk().to_Hz());
        task1::spawn_after(1.secs()).ok();
        (Shared {}, Local { rtc }, init::Monotonics(mono))
    }

    // The idle function is called when there is nothing else to do
    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    // The task functions are called by the scheduler
    #[task(local=[rtc])]
    fn task1(ctx: task1::Context) {
        let time = {
            ctx.local
                .rtc
                .get_datetime()
                .assume_offset(offset!(UTC))
                .unix_timestamp()
        };
        defmt::info!("{:b}", time);
        task1::spawn_after(5.secs()).ok();
    }
}
