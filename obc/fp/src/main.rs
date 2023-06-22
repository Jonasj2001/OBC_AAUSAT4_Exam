#![no_main]
#![no_std]

//use stm32f446_rtic as _; // global logger + panicking-behavior + memory layout
use rtic_playtime::{self as _}; // global logger + panicking-behavior + memory layout
mod id_manager;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1, USART2, USART3,USART6,UART4])]
mod app {
    use crate::id_manager::{self, FP_task_id_manager};

    extern "Rust" {
        #[task(shared = [flash, next_address_id],priority=2)]
        fn FP_task_id_manager(_ctx: FP_task_id_manager::Context);
    }

    //The size of a task in bytes in memory
    pub const TASK_SIZE: u32 = 256; //in bytes
    pub const MAX_NR_OF_TASKS: usize = 48; //3 sectors
    pub const FP_START_ID: u32 = 0x000000; //Start ID og the FP

    //Sets up the stuff above for flash
    #[derive(defmt::Format)]
    pub enum FpConfig {
        TaskSize = 256,
        TaskNum = MAX_NR_OF_TASKS as isize,
        StartAddress = 0x0,
    }

    //START OF RTIC CODE!
    use bxcan::filter::Mask32;
    use bxcan::Fifo;
    use dwt_systick_monotonic::{DwtSystick, ExtU32};

    use flash::w25q128::Memory;
    use heapless::Vec;
    use rtic_playtime::excan::excan::{self as ec};
    use rtic_playtime::exrtc::exrtc::{self as er};
    use rtic_playtime::flightplanner::flightplanner::{self as fp};
    use stm32f4xx_hal::gpio::PushPull;
    use stm32f4xx_hal::{
        can::Can,
        gpio::{
            gpioa::{PA11, PA12, PA5, PA6, PA7},
            Alternate,
        },
        pac::CAN1,
        pac::SPI1,
        prelude::*,
        rtc::Rtc,
        {self as hal},
    };
    use time::macros::{date, time};

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<180_000_000>; // 180 MHz

    #[shared]
    struct Shared {
        can1: bxcan::Can<Can<CAN1, (PA12<Alternate<9>>, PA11<Alternate<9>>)>>,
        first_five: fp::FirstFive,
        next_address_id: Result<u32, id_manager::Error>, //@TODO: Overtages af mem
        flash: Memory<
            SPI1,
            (PA5<Alternate<5>>, PA6<Alternate<5>>, PA7<Alternate<5>>),
            'B',
            6,
            PushPull,
        >,
        rtc: er::RTCSTRUCT,
        can_reply: u8, // mutex for can replys to tasks
    }

    #[local]
    struct Local {
        can_input: Vec<[u8; 8], 32>, //Stores incomming messages over multiple frames
        can_output: Vec<[u8; 8], 32>, //Stores outgoing messages over multiple frames
        fragment_count: u8,          //Counts the number of frames in a message
        current_alarm_time: i32,
    }

    // The init function is called in the beginning of the program
    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::debug!("init");
        // Cortex-M peripherals
        let mut _core: cortex_m::Peripherals = ctx.core;
        // Device specific peripherals
        let mut _device: stm32f4xx_hal::pac::Peripherals = ctx.device;

        /**********************************************************************
        CLOCK SETUP
        ***********************************************************************/
        let rcc = _device.RCC.constrain();
        //External clock: 8MHz, PLL and VCO'ed to System clock: 180MHz,
        //descaled to 90MHz for APB2(Pheriphial clock  2) and 45MHz for APB1(Pheriphial clock  2)
        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(180.MHz())
            .pclk1(45.MHz())
            .pclk2(90.MHz())
            .freeze();

        defmt::debug!("AHB1 clock: {} Hz", clocks.hclk().to_Hz());
        defmt::debug!("APB1 clock: {} Hz", clocks.pclk1().to_Hz());
        // enable tracing and the cycle counter for the monotonic timer
        _core.DCB.enable_trace();
        _core.DWT.enable_cycle_counter();

        // Set up the monotonic timer
        let mono = DwtSystick::new(&mut _core.DCB, _core.DWT, _core.SYST, clocks.hclk().to_Hz());
        /****************************************s******************************
        END OF CLOCK SETUP
        ***********************************************************************/

        //splits pins for easier access
        let gpioa = _device.GPIOA.split();
        let gpiob = _device.GPIOB.split();

        /**********************************************************************
        CAN SETUP
        ***********************************************************************/
        let mut can1 = {
            // CAN pins alternate function 9 as per datasheet
            // https://www.st.com/resource/en/datasheet/stm32f446mc.pdf page 57
            let rx = gpioa.pa11.into_alternate::<9>();
            let tx = gpioa.pa12.into_alternate::<9>();

            let can = _device.CAN1.can((tx, rx));

            defmt::debug!("CAN1, waiting for 11 recessive bits...");
            bxcan::Can::builder(can)
                // APB1 (PCLK1): 45MHz, Bit rate: 1MBit/s, Sample Point 87.5%
                // Value was calculated with http://www.bittiming.can-wiki.info/
                .set_bit_timing(0x001b0002)
                .set_automatic_retransmit(true)
                .enable()
        };
        defmt::debug!("CAN1, waiting for 11 recessive bits... (done)");

        can1.enable_interrupts({
            use bxcan::Interrupts as If;
            If::FIFO0_MESSAGE_PENDING | If::FIFO0_FULL | If::FIFO0_OVERRUN
        });

        // Configure filters so that can frames can be received - should be configured for
        can1.modify_filters()
            .enable_bank(0, Fifo::Fifo0, Mask32::accept_all());
        let can_input = Vec::<[u8; 8], 32>::new();
        let can_output = Vec::<[u8; 8], 32>::new();

        /**********************************************************************
        END OF CAN SETUP
        ***********************************************************************/

        /**********************************************************************
        MEM SETUP
        ***********************************************************************/
        //Setup pins
        let sclk = gpioa
            .pa5
            .into_alternate()
            .speed(stm32f4xx_hal::gpio::Speed::VeryHigh);
        let miso = gpioa
            .pa6
            .into_alternate()
            .speed(stm32f4xx_hal::gpio::Speed::VeryHigh);
        let mosi = gpioa
            .pa7
            .into_alternate()
            .speed(stm32f4xx_hal::gpio::Speed::VeryHigh);
        let mut cs = gpiob.pb6.into_push_pull_output();
        cs.set_high(); //Device active low.

        //Settings for SPI mode, Polarity and phase.
        let spi_mode = hal::spi::Mode {
            polarity: hal::spi::Polarity::IdleLow,
            phase: hal::spi::Phase::CaptureOnFirstTransition,
        };

        let spi = _device.SPI1.spi(
            (sclk, miso, mosi), //Settings SPI pins
            spi_mode,           //Setting Mode
            10.MHz(),           //Setting clock
            &clocks,            //Give a reference to system clocks.
        );
        let flash = Memory::new_w25q128(spi, cs);

        #[cfg(feature = "clean")]
        flash.delete(Flash::w25q128::Delete::BlockErase64, 0x00);
        /**********************************************************************
        END OF MEM SETUP
        ***********************************************************************/
        //Sets up the first five vector - used for keeping track on the next upcoming tasks.
        let first_five = fp::FirstFive::new();

        // RTC SETUPS
        let mut rtc = Rtc::new_lsi(_device.RTC, &mut _device.PWR);
        rtc.set_date(&date!(1970 - 01 - 01)).unwrap();
        rtc.set_time(&time!(00:00:00)).unwrap();

        //Configures the first alarm
        let first_alarm: i32 = 50;
        let current_alarm_time = first_alarm;
        //Inistialises the alarm part of the RTC
        let rtc = er::RTCSTRUCT::new(_device.EXTI, _device.PWR, rtc, first_alarm);

        //Finally, the initialization of the first five vector.
        FP_sort_first_five_full::spawn().ok();
        defmt::debug!("Init done!");
        ping::spawn().ok();
        (
            Shared {
                can1,
                first_five,
                next_address_id: Result::Ok(0),
                flash,
                rtc,
                can_reply: 0,
            },
            Local {
                can_input,
                can_output,
                fragment_count: 0,
                current_alarm_time,
            },
            init::Monotonics(mono),
        )
    }

    // The idle function is called when there is nothing else to do
    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    // The task functions are called by the scheduler
    #[task(shared = [rtc])]
    fn ping(_ctx: ping::Context) {
        let mut rtc = _ctx.shared.rtc;
        let time = rtc.lock(|r| r.get_time(false));
        defmt::debug!("RTC: {}", time);
        ping::spawn_after(5.secs()).ok();
    }

    //Task for sending messages over CAN.
    #[task(shared = [can1], local=[can_output, fragment_count], priority=5)]
    fn can_send(
        ctx: can_send::Context,
        priority: u8,
        receiver: u8,
        port: u8,
        cmd: u8,
        message_content: Vec<[u8; 8], 32>,
        new_message: bool,
    ) {
        let mut can = ctx.shared.can1;
        let frg_cnt = ctx.local.fragment_count;
        let msg_queue = ctx.local.can_output;
        let mut start_bit: bool = false;
        let mut end_bit: bool = false;
        if new_message {
            //Sets up new message: High start bit, count of 0, and puts frames in queue
            *frg_cnt = 0;
            start_bit = true;
            msg_queue.clear();
            msg_queue.extend(message_content.into_iter());
        }

        while !msg_queue.is_empty() || start_bit {
            let data: [u8; 8] = {
                if !msg_queue.is_empty() {
                    msg_queue.remove(0)
                } else {
                    [0; 8]
                }
            };
            defmt::debug!("Sending: {:?}", data);
            if msg_queue.is_empty() {
                end_bit = true;
            };
            let frame = ec::build_id(
                priority, receiver, port, cmd, start_bit, end_bit, *frg_cnt, &data,
            );

            //Waits for transmitter to be idle - can be disabled for testing without receiver
            //Otherwise it would just dequeue the message, and not send it.
            loop {
                if can.lock(|c| c.is_transmitter_idle()) {
                    break;
                };
            }

            //Nessecary delay for CAN to be able to resend message in case of error
            //1000: good for 1Mbit/s - Equiv of 900us delay
            // for _i in 0..1000 {
            //     continue;
            // }

            loop {
                if can.lock(|c| c.transmit(&frame).is_ok()) {
                    break;
                } else {
                    defmt::debug!("Transmit failed!");
                }
            }
            *frg_cnt += 1;
            start_bit = false;
        }
    }

    // receive a message via CAN
    #[task(binds = CAN1_RX0,shared = [can1], local=[can_input],priority=6)]
    fn can_receive(ctx: can_receive::Context) {
        let mut can1 = ctx.shared.can1;
        let can_input = ctx.local.can_input;
        let frame = can1.lock(|can1| can1.receive().unwrap());
        /*@TODO: Add functionality that checks that the ID doesn't change (except SB, EB & frg_cnt) between messages. */

        //Initializes frame_id as a struct, to split the frame
        let frame_id = ec::IdentifierContents::frame_splitter(&frame);

        //Checks if the message is for us
        if (0 == frame_id.rec) || (1 == frame_id.rec) {
            if frame_id.start_bit {
                can_input.clear();
            }

            //Data is removed from frame.data, over in our own array.
            //Should probably be done in a better way, but this works for now.
            let data: [u8; 8] = {
                let frame_data = frame.data().unwrap();
                let mut storage: [u8; 8] = [0; 8];
                for x in 0..frame_data.len() {
                    storage[x] = frame_data[x]
                }
                storage
            };

            let mut reply = Vec::<[u8; 8], 32>::new();
            can_input.push(data).ok();
            if frame_id.end_bit {
                if can_input.is_full() && (!(data[6] == 0x00) || !(data[7] == 0x00)) {
                    //If this is the last bit of data - push it to the right recipient
                    defmt::debug!("Fragment count is 31, but end bit is not set!");
                    reply
                        .push([0x15, 0x44, 0x61, 0x74, 0x32, 0x4C, 0x6E, 0x67])
                        .ok();
                    can_send::spawn(3, 2, 0, 0, reply, true).ok();
                }
                match frame_id.port {
                    //Port 0-2: Network protocols
                    0..=2 => defmt::todo!("Network protocols not implemented"),
                    //Port 3: Flight Planner
                    3 => match Flight_Planner::spawn(frame_id, can_input.clone()) {
                        Err(_) => defmt::error!("Flight Planner task not spawned"),
                        Ok(_) => defmt::debug!("Flight Planner task spawned"),
                    },
                    //Port 4: Log
                    4 => defmt::debug!("Log not implemented"),
                    //Port 5: RTC
                    5 => match RTC_get_time::spawn() {
                        Err(_) => defmt::error!("RTC task not spawned"),
                        Ok(_) => defmt::debug!("RTC task spawned"),
                    },
                    //Port 3-7: Ikke implementeret.
                    _ => defmt::debug!("Port {} not implemented", frame_id.port),
                };
            } else {
                if can_input.is_full() {
                    //If this is the last bit of data - push it to the right recipient
                    defmt::debug!("Fragment count is 31, but end bit is not set!");
                    reply
                        .push([0x15, 0x4D, 0x73, 0x67, 0x32, 0x4C, 0x6E, 0x67])
                        .ok();
                    can_send::spawn(3, 2, 0, 0, reply, true).ok();
                }
            };
        } else {
            defmt::debug!("Message not 4 us");
        }
        //defmt::debug!("Can receive done");
    }

    // The task functions are called by the scheduler
    #[task(shared = [rtc])]
    fn RTC_get_time(_ctx: RTC_get_time::Context) {
        let mut rtc = _ctx.shared.rtc;
        let t = rtc.lock(|r| r.get_time(false)) as i32;
        let mut data = Vec::<[u8; 8], 32>::new();
        data.push((t as u64).to_be_bytes()).ok();
        can_send::spawn(3, 2, 0, 0, data, true).ok();
    }

    #[task(priority = 3, capacity = 3)] //Determines command and sends it to the right task
    fn Flight_Planner(
        _ctx: Flight_Planner::Context,
        frame_id: ec::IdentifierContents,
        data: Vec<[u8; 8], 32>,
    ) {
        match frame_id.cmd {
            //CMD 0: Reply
            0 => FP_read_reply::spawn(data[0][0]).ok(),
            //CMD 1: Requestd
            1 => match data[0][0] {
                //Case: 0x35 (ascii '5') - Send first five
                0x35 => FP_request_ff::spawn().ok(),
                //Case_ 0x46 (ascii 'F') - Send Full list
                0x46 => FP_request_schedule::spawn().ok(),
                //Default: Send error, 0x15 (ascii 'NAK', Not Acknowledged)
                _ => {
                    let mut reply = Vec::<[u8; 8], 32>::new();
                    reply
                        .push([0x15, 0x57, 0x72, 0x6E, 0x67, 0x44, 0x61, 0x74])
                        .ok();
                    can_send::spawn(3, 2, 0, 0, reply, true).ok()
                }
            },
            //CMD 2: Schedule task
            2 => FP_schedule_task::spawn(data, false).ok(),
            //CMD 3: Alter
            3 => FP_alter_task::spawn(data).ok(),
            //CMD 4: Delete
            4 => {
                let address: u32 = u32::from_be_bytes([0, 0, data[0][0], data[0][1]]);
                FP_delete_task::spawn(address, true).ok()
            }
            //CMD 3-255: Not implemented - try_into().ok() to
            _ => defmt::debug!("CMD {} has not been implemented", frame_id.cmd)
                .try_into()
                .ok(),
        };
    }

    #[task(shared=[can_reply],priority=3)] //Alter Task
    fn FP_read_reply(ctx: FP_read_reply::Context, data: u8) {
        //simply placeses response in reply variable
        let mut reply = ctx.shared.can_reply;
        reply.lock(|can_reply| *can_reply = data);
    }

    #[task(shared=[flash])] //Request Schedule
    fn FP_request_schedule(ctx: FP_request_schedule::Context) {
        defmt::debug!("Full schedule has been requested!");
        let mut flash = ctx.shared.flash;
        //Get the addresses of the task list
        let mut executed_list = Vec::<u32, MAX_NR_OF_TASKS>::new();
        for i in 0..FpConfig::TaskNum as u32 {
            let address = (FpConfig::StartAddress as u32 + i as u32) * FpConfig::TaskSize as u32;
            let mut executed_byte: [u8; 1] = [0; 1];
            //Executed is in byte 3 - thus address + 2
            flash.lock(|f| f.read(address + 2, 1, &mut executed_byte));
            //If a task is scheduled: 0bxx001111, if it is executed: 0bxx000101
            if fp::is_execute_ready(executed_byte[0]) {
                executed_list.push(address).ok();
            }
        }

        //No sorting implemented yet
        if !executed_list.is_empty() {
            for i in executed_list.iter() {
                let address = *i;
                let mut flash_task: [u8; 256] = [0; 256];
                flash.lock(|f| f.read(address, flash_task.len(), &mut flash_task));

                let data_vec = fp::decompile_task(&mut flash_task, address);

                loop {
                    let err = can_send::spawn(3, 2, 0, 0, data_vec.clone(), true).is_err();
                    if !err {
                        break;
                    }
                }
            }
        }
        let mut reply = Vec::<[u8; 8], 32>::new();
        reply.push([0x17, 0, 0, 0, 0, 0, 0, 0]).ok();
        //Send a acknowledgement that everything has been sent
        loop {
            let err = can_send::spawn(3, 2, 0, 0, reply.clone(), true).is_err();
            if !err {
                break;
            }
        }
    }

    #[task(shared=[first_five,flash])] //Request Schedule
    fn FP_request_ff(ctx: FP_request_ff::Context) {
        defmt::debug!("First Five has been requested!");
        let mut flash = ctx.shared.flash;
        let mut ff = ctx.shared.first_five;
        //Case: 0x35 (ascii '5') - Send first five
        //Case_ 0x46 (ascii 'F') - Send Full list
        defmt::debug!("Print first five");
        let mut ffl = Vec::<fp::FFArray, 5>::new();
        ffl.extend(ff.lock(|f| f.content.clone()).into_iter());
        for i in 0..ffl.len() {
            let ff_task = ffl[i];
            defmt::debug!("Sending task: {}", ff_task.id);
            let mut task: [u8; 256] = [0; 256];
            flash.lock(|f| f.read(ff_task.id, (ff_task.dlc * 8) as usize, &mut task));

            let data_vec = fp::decompile_task(&mut task, ff_task.id);

            loop {
                if can_send::spawn(3, 2, 0, 0, data_vec.clone(), true).is_ok() {
                    defmt::debug!("Task {} sent!", ff_task.id);
                    break;
                }
            }
        }

        let mut reply = Vec::<[u8; 8], 32>::new();
        reply.push([0x17, 0, 0, 0, 0, 0, 0, 0]).ok();
        //Send a acknowledgement that everything has been sent
        loop {
            let err = can_send::spawn(3, 2, 0, 0, reply.clone(), true).is_err();
            if !err {
                break;
            }
        }
    }

    #[task(shared=[first_five],priority = 3)] //Kodestub til at læse og sortere first_five
    fn FP_sort_first_five_quick(
        ctx: FP_sort_first_five_quick::Context,
        id: u32,
        execution_time: i32,
        priority: u8,
        dlc: u8,
    ) {
        defmt::debug!("Sorting first five Quick");
        let mut firstfive = ctx.shared.first_five;
        let firstfiveclone = firstfive.lock(|first_five| first_five.clone());

        //Creates a temporary 6 element vector for the original First Five + the new element
        let mut ff = Vec::<fp::FFArray, 6>::new();
        ff.extend(firstfiveclone.content.into_iter());

        let ff = {
            ff.push(fp::FFArray {
                id,
                execution_time,
                priority,
                dlc,
            })
            .ok();
            //sorts by time and priority and returns the first five
            fp::sort_to_ff(&ff)
        };

        //Set a new alarm - if nothing is in list, disable alarm
        if ff.is_empty() {
            FP_set_alarm::spawn(-1).ok();
        } else {
            FP_set_alarm::spawn(ff[0].execution_time).ok();
            defmt::debug!("SFFQ: Alarm set to: {}", ff[0].execution_time);
        }

        defmt::debug!("Updating first five");
        firstfive.lock(|firstfive| {
            firstfive.update(ff);
        });
        defmt::debug!("Update done");
    }

    #[task(shared=[first_five,flash],priority = 3)]
    fn FP_sort_first_five_full(ctx: FP_sort_first_five_full::Context) {
        let mut firstfive = ctx.shared.first_five;
        let mut flash = ctx.shared.flash;

        let mut executed_list = Vec::<u32, MAX_NR_OF_TASKS>::new();
        for i in 0..MAX_NR_OF_TASKS {
            let address = (FpConfig::StartAddress as u32 + i as u32) * FpConfig::TaskSize as u32;
            let mut executed_byte: [u8; 1] = [0; 1];
            //Executed ligger i byte nr 3 (derad + 3)
            flash.lock(|f| f.read(address + 2, 1, &mut executed_byte));
            //Hvis en task er schedules: 0bxx001111, hvis den er executed: 0bxx000101
            if fp::is_execute_ready(executed_byte[0]) {
                executed_list.push(address).ok();
            }
        }

        let mut full_task_list = Vec::<fp::FFArray, MAX_NR_OF_TASKS>::new();
        if !executed_list.is_empty() {
            for i in executed_list.iter() {
                let address = *i;
                let mut flash_task: [u8; 8] = [0; 8];
                flash.lock(|f| f.read(address, 8, &mut flash_task));

                let priority: u8 = flash_task[0] >> 5;
                let execution_time: i32 = i32::from_be_bytes([
                    flash_task[3],
                    flash_task[4],
                    flash_task[5],
                    flash_task[6],
                ]);
                let dlc = flash_task[7];

                full_task_list
                    .push(fp::FFArray {
                        id: address,
                        execution_time: execution_time,
                        priority: priority,
                        dlc,
                    })
                    .ok();
            }
        }
        let ff = fp::sort_to_ff(&full_task_list);

        //Set a new alarm - if nothing is in list, disable alarm
        if ff.is_empty() {
            FP_set_alarm::spawn(-1).ok();
        } else {
            defmt::debug!("SFFF: Alarm set to: {}", ff[0].execution_time);
            FP_set_alarm::spawn(ff[0].execution_time).ok();
        };

        fp::print_ff(&ff);
        firstfive.lock(|firstfive| {
            firstfive.update(ff);
        });
    }

    #[task(shared=[rtc],local=[current_alarm_time],priority=3)] //Task til at skabe en addresse - @TODO: slet blokke :) - Sæt en stopklods
    fn FP_set_alarm(ctx: FP_set_alarm::Context, alarm_time: i32) {
        defmt::debug!("Set alarm to: {}", alarm_time);
        let mut rtc = ctx.shared.rtc;
        let cat = ctx.local.current_alarm_time;

        //Skab lokal tidstjek
        if !(alarm_time == *cat) {
            if alarm_time < 0 {
                rtc.lock(|rtc| rtc.disable_alarm());
                *cat = -1;
            } else {
                rtc.lock(|rtc| {
                    rtc.set_alarm_time(alarm_time);
                    *cat = alarm_time;
                });
            }
        }
        defmt::debug!("Set alarm Success!");
    }

    #[task(shared=[])] //Alter Task
    fn FP_alter_task(_ctx: FP_alter_task::Context, data: Vec<[u8; 8], 32>) {
        /*@TODO: Implement a way  to quick sort a task that fits in the FF.*/
        defmt::debug!("Begun Alter Task");
        let mut new_data = Vec::<[u8; 8], 32>::new();
        new_data.extend(data.into_iter());
        let address: u32 = ((new_data[1][0] as u32) << 8) | new_data[1][1] as u32;
        for i in 1..new_data.len() {
            if i + 1 == new_data.len() {
                new_data[i][0] = 0;
                new_data[i][1] = 0;
            } else {
                new_data[i][0] = new_data[i + 1][0];
                new_data[i][1] = new_data[i + 1][1];
            }
            new_data[i].rotate_left(2);
        }

        for i in 0..new_data.len() {
            defmt::debug!("data[{}]: {:?}", i, new_data[i]);
        }
        defmt::debug!("data lenght: {}", new_data.len());
        FP_delete_task::spawn(address, false).ok();
        FP_schedule_task::spawn(new_data, true).ok();
    }

    #[task(shared=[flash])] //Delete Task
    fn FP_delete_task(ctx: FP_delete_task::Context, address: u32, respond: bool) {
        defmt::debug!("Begun Delete Task");
        let mut flash = ctx.shared.flash;
        flash.lock(|f| f.write(address + 2, &[0b00000101]));
        defmt::debug!("Task {} has been deleted!", address);
        if respond {
            FP_sort_first_five_full::spawn().ok();
            let mut reply = Vec::<[u8; 8], 32>::new();
            reply.push([0x06, 0, 0, 0, 0, 0, 0, 0]).ok();
            can_send::spawn(3, 2, 0, 0, reply, true).ok();
        }
    }

    #[task(shared = [next_address_id,flash,rtc])]
    fn FP_schedule_task(
        ctx: FP_schedule_task::Context,
        data: Vec<[u8; 8], 32>,
        is_alter_trigger: bool,
    ) {
        let dlc: u8 = data.len() as u8;
        defmt::debug!("data lenght: {}", dlc);
        //WHEN SENDING TO SCHEDULE TASK, THE FIRST CAN PACKAGE MUST be:
        //| 1B priority | 1B receiver| 1B port | 1B command | 4B execution time |
        //An address ID is collected form the adress manager task, and collected from the shared(mutex)
        FP_task_id_manager::spawn().unwrap();
        let mut id_man = ctx.shared.next_address_id;
        let mut flash = ctx.shared.flash;
        let mut rtc = ctx.shared.rtc;

        let mut integrety_check: bool = false;
        let mut bad_time: bool = false;
        let mut fp_full: bool = false;

        let mut reply = Vec::<[u8; 8], 32>::new();

        let address = id_man.lock(|id_man| *id_man);
        fp_full = address.is_err();
        if !fp_full {
            let address = address.unwrap();

            //Task array is initialized, filled with zeros - NOTE: Array is used to make space for Executed byte at the end.
            let task = fp::compile_task(&data, false);
            //Sorts the new task into the list
            let priority: u8 = data[0][0];
            let exe_time: i32 =
                i32::from_be_bytes([data[0][4], data[0][5], data[0][6], data[0][7]]);

            let current_time: i32 = rtc.lock(|r| r.get_time(false)) as i32;
            bad_time = !(exe_time > current_time);

            integrety_check = {
                if !(bad_time || fp_full) {
                    //Writes the task to memory
                    flash.lock(|f| f.write(address, &task));
                    //Reads the task back from memory, for confirmation of task
                    let mut read_back_content: [u8; 256] = [0; 256];
                    flash.lock(|f| f.read(address, (dlc * 8) as usize, &mut read_back_content));
                    defmt::debug!("Read back content: {:?}", read_back_content[0..8]);

                    fp::compare_tasks(&task, &read_back_content)
                } else {
                    false
                }
            };
            //Test the read back - Success or fail?
            reply
                .push({
                    if integrety_check {
                        defmt::debug!("Task has succesfully been written to memory!");
                        let add = address.to_be_bytes();
                        [0x06, 0, 0, 0, 0, 0, add[2], add[3]]
                    } else {
                        if bad_time {
                            defmt::debug!("Invalid time! Time has happend!");
                            [0x15, 0x57, 0x72, 0x6E, 0x67, 0x54, 0x69, 0x6D]
                        } else {
                            defmt::debug!("Something went wrong when writing to memory!");
                            [0x15, 0x42, 0x61, 0x64, 0x57, 0x72, 0x69, 0x74]
                        }
                    }
                })
                .ok();
            if !bad_time {
                defmt::debug!("Time officially good");
                //Checks if task belongs in first_five
                if is_alter_trigger {
                    FP_sort_first_five_full::spawn().ok();
                } else {
                    FP_sort_first_five_quick::spawn(address, exe_time, priority, dlc).ok();
                }
            };
        } else {
            defmt::debug!("No more addresses available!");
            reply
                .push([0x15, 0x46, 0x50, 0x20, 0x46, 0x75, 0x6C, 0x6C])
                .ok();
        }

        //Sends response
        //prio, rec, port, cmd, data, true
        //rec 2: radio
        loop {
            if can_send::spawn(7, 2, 0, 0, reply.clone(), true).is_ok() {
                break;
            }
        }
    }

    #[task(binds = RTC_ALARM)]
    fn FP_execute_trigger_task(_ctx: FP_execute_trigger_task::Context) {
        defmt::debug!("RTC triggered");
        FP_execute_task::spawn().ok();
    }

    #[task(shared=[first_five,rtc,flash,can_reply], priority = 2)] //local = [exe_spawn])] //execute_task
    fn FP_execute_task(ctx: FP_execute_task::Context) {
        let mut ffs = ctx.shared.first_five;
        let mut rtc = ctx.shared.rtc;

        let ff = ffs.lock(|ff| ff.clone());
        if ff.content.is_empty() {
            defmt::debug!("FP is empty");
            FP_set_alarm::spawn(-1).ok();
        } else {
            let mut flash = ctx.shared.flash;
            let mut firsttask = ff.content[0];
            let time = rtc.lock(|rtc| rtc.get_time(false)) as i32;

            //Checks for time
            if time >= firsttask.execution_time {
                //Request time from memory
                defmt::debug!("Time to execute task {} at time {}", firsttask.id, time);
                let mut task: [u8; 256] = [0; 256];
                flash.lock(|f| f.read(firsttask.id, (firsttask.dlc * 8) as usize, &mut task));
                let non_executed_byte = task[2];
                defmt::debug!("read task: {:?}", task);
                let executed_byte = non_executed_byte & 0b11000101;
                let can_id = u32::from_be_bytes([0, task[0], task[1], task[2]]);
                let (prio, rec, port, cmd) = (
                    ((can_id >> 21) as u8 & 7),
                    ((can_id >> 17) as u8 & 15),
                    ((can_id >> 14) as u8 & 7),
                    ((can_id >> 6) as u8 & 255),
                );
                defmt::debug!("prio: {}, rec: {}, port: {}, cmd: {}", prio, rec, port, cmd);
                let mut data = Vec::<[u8; 8], 32>::new();
                let dlc: usize = task[7] as usize;
                defmt::debug!("Executed task has a dlc of {}", dlc);

                for i in 1..dlc {
                    defmt::debug!("packing message {}", i);
                    //Start compiling at task nr 8
                    let mul = i * 8;
                    //package byte [8-15][16...]...
                    let package: [u8; 8] = [
                        task[(mul) as usize],
                        task[(mul + 1) as usize],
                        task[(mul + 2) as usize],
                        task[(mul + 3) as usize],
                        task[(mul + 4) as usize],
                        task[(mul + 5) as usize],
                        task[(mul + 6) as usize],
                        task[(mul + 7) as usize],
                    ];
                    data.push(package).ok();
                }
                defmt::debug!("Data vec lenght: {:?}", data.len());

                //TRANSMIT CAN
                can_send::spawn(prio, rec, port, cmd, data, true).ok();

                //RECEIVE ACKNOWLEDGEMENT
                let mut reply_ctx = ctx.shared.can_reply;
                defmt::debug!("Waiting for reply");
                loop {
                    let reply = reply_ctx.lock(|reply| *reply);
                    if reply == 0x06 {
                        defmt::debug!("Task {} executed!", firsttask.id);
                        reply_ctx.lock(|reply| *reply = 0);
                        break;
                    }
                }
                //Write executed byte to memory
                flash.lock(|f| f.write(firsttask.id + 2, &[executed_byte]));
                FP_sort_first_five_full::spawn().ok();
            }

            //Checks if there is a next task - If there is, check for time, if not, disable alarm
            if ff.content.len() > 1 {
                firsttask = ff.content[1];
                let time_to_next: u32 = {
                    match firsttask.execution_time.checked_sub(time) {
                        Some(X) => match X {
                            i32::MIN..=0 => 0 as u32, //If time is negative, set to 0
                            1..=i32::MAX => X as u32,
                        },
                        None => 0,
                    }
                };
                defmt::debug!("Time to next task: {}", time_to_next);
                if 1 > time_to_next {
                    rtc.lock(|f| f.disable_alarm());
                    FP_execute_task::spawn().unwrap();
                };
            } else {
                FP_set_alarm::spawn(-1).ok();
            }
        }
    }
}
