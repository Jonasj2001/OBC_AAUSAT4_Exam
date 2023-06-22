#![no_main]
#![no_std]

//use stm32f446_rtic as _; // global logger + panicking-behavior + memory layout
use rtic_playtime::{self as _}; // global logger + panicking-behavior + memory layout

pub struct CanData {
    pub content: [[u8; 8]; 32],
    pub count: u8,
}

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1, USART2,USART3, USART6])]
mod app {

    use bxcan::filter::Mask32;
    use bxcan::Fifo;
    use dwt_systick_monotonic::{DwtSystick, ExtU32};
    use heapless::Vec;
    use rtic_playtime::excan::excan::{self as ec};
    //use rtic_playtime::exrtc::exrtc;
    use stm32f4xx_hal::{
        can::Can,
        gpio::{
            gpioa::{PA11, PA12, PA5},
            Alternate, Output, PushPull,
        },
        pac::CAN1,
        prelude::*,
        rtc::Rtc,
    };
    use time::macros::{date, offset, time};

    // Needed for scheduling monotonic tasks
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<180_000_000>; // 180 MHz

    // Holds the shared resources (used by multiple tasks)
    // Needed even if we don't use it
    #[shared]
    struct Shared {
        //can1 opsættes til den interne CAN1, og bliver linket til PA12 og PA11 på den alternative funktion 9.
        can1: bxcan::Can<Can<CAN1, (PA12<Alternate<9>>, PA11<Alternate<9>>)>>,
        sharedtime: [u8; 8],
        sharedtaskid: Vec<[u8; 8], 10>,
        data_from_can: Vec<[u8; 8], 32>,
        rtc: Rtc<stm32f4xx_hal::rtc::Lsi>,
        local_tasklist: Vec<Vec<[u8; 8], 32>, 48>,
    }

    // Holds the local resources (used by a single task)
    // Needed even if we don't use it
    #[local]
    struct Local {
        led: PA5<Output<PushPull>>,
        can_input: Vec<[u8; 8], 32>,
        can_output: Vec<[u8; 8], 32>, //Stores outgoing messages over multiple frames
        fragment_count: u8,
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

        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(180.MHz())
            .pclk1(45.MHz())
            .pclk2(90.MHz())
            .freeze();

        // enable tracing and the cycle counter for the monotonic timer
        _core.DCB.enable_trace();
        _core.DWT.enable_cycle_counter();
        //BEGIN RTC SETUP
        let mut rtc = Rtc::new_lsi(_device.RTC, &mut _device.PWR);
        rtc.set_date(&date!(1970 - 01 - 01)).unwrap();
        rtc.set_time(&time!(00:00:00)).unwrap();
        //END RTC SETUP

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
                // .set_bit_timing(0x001b0002)
                .set_bit_timing(0x00390002)
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

        //initiate local and shared elements
        let can_output = Vec::<[u8; 8], 32>::new();
        let fragment_count = 0;
        let can_input = Vec::<[u8; 8], 32>::new();
        let data_from_can = Vec::<[u8; 8], 32>::new();
        let sharedtaskid = Vec::<[u8; 8], 10>::new();
        let local_tasklist = Vec::<Vec<[u8; 8], 32>, 48>::new();

        // Set up the monotonic timer
        let mono = DwtSystick::new(&mut _core.DCB, _core.DWT, _core.SYST, clocks.hclk().to_Hz());

        defmt::info!("Init done!");
        blink::spawn_after(2.secs()).ok();
        get_time::spawn_after(1.secs()).ok();
        (
            Shared {
                can1,
                sharedtime: [0; 8],
                sharedtaskid,
                data_from_can,
                rtc,
                local_tasklist,
            },
            Local {
                led,
                can_output,
                fragment_count,
                can_input,
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
    #[task(local = [led], shared = [rtc])]
    fn blink(mut ctx: blink::Context) {
        blink::spawn_after(1.secs()).ok();
        ctx.local.led.toggle();
        defmt::info!(
            "time: {}",
            ctx.shared.rtc.lock(|c| c
                .get_datetime()
                .assume_offset(offset!(UTC))
                .unix_timestamp())
        );
    }

    // send a meesage via CAN
    #[task(shared = [can1], local =[can_output, fragment_count], priority=10)]
    fn can_send(
        ctx: can_send::Context,
        priority: u8,
        receiver: u8,
        port: u8,
        cmd: u8,
        data: Vec<[u8; 8], 32>,
        new_message: bool,
    ) {
        //defmt::info!("can send initiate");
        let mut start_bit: bool = false;
        let mut end_bit: bool = false;
        let mut can = ctx.shared.can1;
        let msg_queue = ctx.local.can_output;
        let frg_cnt = ctx.local.fragment_count;
        //defmt::info!("datalength: {}", data.len());
        if new_message {
            *frg_cnt = 0;
            start_bit = true;
            msg_queue.clear();
            msg_queue.extend(data.into_iter());
        } else {
            // *frg_cnt+=1;
        };
        while !msg_queue.is_empty() || start_bit {
            //defmt::info!("Starting sending f: {}", frg_cnt);
            // if CAN_QUEUE_PACKAGES{
            loop {
                if can.lock(|c| c.is_transmitter_idle()) {
                    break;
                };
            }
            //defmt::info!("Is idle!");
            // }
            let data: [u8; 8] = {
                if !msg_queue.is_empty() {
                    msg_queue.remove(0)
                } else {
                    [0; 8]
                }
            };
            if msg_queue.is_empty() {
                end_bit = true;
            };

            let frame = ec::build_id(
                priority, receiver, port, cmd, start_bit, end_bit, *frg_cnt, &data,
            );

            //defmt::info!("Sending frame with data: {:#06X}", data);
            for _i in 0..1000 {
                continue;
            }
            loop {
                if can.lock(|c| c.transmit(&frame).is_ok()) {
                    break;
                } else {
                    defmt::debug!("Transmit failed!");
                }
            }
            //defmt::info!("Frame sent!{}", frg_cnt);
            *frg_cnt += 1;
            start_bit = false;
        }
    }

    // receive a message via CAN
    #[task(binds = CAN1_RX0, shared = [can1,sharedtime, sharedtaskid, data_from_can, rtc, local_tasklist], local=[can_input],priority=4)]
    fn can_receive(mut ctx: can_receive::Context) {
        //defmt::info!("received message");
        let mut can1 = ctx.shared.can1;
        let can_input = ctx.local.can_input;
        let frame = can1.lock(|can1| can1.receive().unwrap());

        //Initializes frame_id as a struct, to split the frame
        let frame_id = ec::IdentifierContents::frame_splitter(&frame);

        //Checks if the message is for us
        if (0 == frame_id.rec) || (2 == frame_id.rec) {
            //defmt::info!("message for us");
            //frame_id.print();

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

            can_input.push(data).ok();
            if frame_id.end_bit {
                // for i in 0..can_input.len() {
                //     defmt::info!("Element {} in message: {:#06X}", i, can_input[i]);
                // }
                defmt::info!("end frame");
                //saves the time in shared variable, is only used by set_time task
                let mut st = ctx.shared.sharedtime;
                st.lock(|c| *c = can_input[0]);
                //copy can_input over to shared variable to be processed in the respective tasks
                let mut data_from_can = ctx.shared.data_from_can;
                if frame_id.cmd == 2 {
                    //let task= can_input[0][0] as usize;
                    can_input.clear();
                    defmt::info!(
                        "task is executed at time {}",
                        ctx.shared.rtc.lock(|c| c
                            .get_datetime()
                            .assume_offset(offset!(UTC))
                            .unix_timestamp())
                    );
                    ctx.shared.local_tasklist.lock(|c| c.pop());
                    reply::spawn().ok();
                    request_task::spawn(0x46).ok();
                } else {
                    data_from_can.lock(|c| {
                        c.clear();
                        c.extend(can_input.clone().into_iter())
                    });
                }
            };
        } else {
            defmt::info!("Message not 4 us");
        }
        // if received command = 2 it is seen as a task execute and can_input is cleared in order to free up the locks

        //task list from requested to check for last message
        //defmt::info!("can receive done");
    }

    //send message to rtc to receive the current time in order to schedule tasks
    #[task(shared=[sharedtime, data_from_can], priority=1)]
    fn get_time(mut ctx: get_time::Context) {
        //get time [u8,8] [time, time, time, time, 0, 0, 0, 0]
        let message: [u8; 8] = [0; 8]; //filler data
        let mut sending = Vec::<[u8; 8], 32>::new();
        sending.push(message).ok();

        //CAN header values [priority, receiver, port, command, sb, eb, frg_count]
        let priority: u8 = 5;
        let reciever: u8 = 1; //obc
        let port: u8 = 5; //rtc
        let cmd: u8 = 0; //schedule
        can_send::spawn(priority, reciever, port, cmd, sending, true).ok();
        //defmt::info!("need to sync for time");
        // lock the task untill the time has been received
        let mut data_from_can = ctx.shared.data_from_can;
        loop {
            let current_time: [u8; 8] = ctx.shared.sharedtime.lock(|c| *c);
            if current_time != [0; 8] {
                set_time::spawn().ok();
                //defmt::info!("time has been synced");
                data_from_can.lock(|c| c.clear()); //clear data ready for next receive
                break;
            }
        }
    }

    //need to link receive to the time

    #[task(shared=[sharedtime])]
    fn set_time(mut ctx: set_time::Context) {
        //defmt::info!("set time and initiate tasks");
        let got_time: [u8; 8] = ctx.shared.sharedtime.lock(|c| *c);
        let time: i32 = ((got_time[4] as i32) << 24)
            + ((got_time[5] as i32) << 16)
            + ((got_time[6] as i32) << 8)
            + (got_time[7] as i32);
        defmt::info!("the time is: {}", time);
        //defmt::info!("Time is: {}", time);
        //defmt::info!("schedule task 1");

        /*
        for standard send to radio task, the header is:  [prio, rec, port, cmd, time]
                                                         [ 1  ,  2 ,  0  ,  2 , time]

                                                         cmd 2 = to see that the task has been executed

        schedule task
        schedule_task::spawn(prio, rec, port, cmd, time + offset)).ok();

        size is for either full task list or first five. 0x35 for first five and 0x46 for full task list.

        request task list
        request_task::spawn(size).ok();

        ID is the number in the list aka the amount of times schedule task has run fx, if 3 schedule task/ alter task has run the last task id is 3. if a task is altered or removed the id is removed from the list
        ID is stored in a vector <[u8;8], 10>

        alter task
        alter_task::spawn(ID, prio, rec, port, cmd, time + offset).ok();

        delete task
        delete_task::spawn(ID).ok();
        */
        for i in 0..48 {
            let data1: [u8; 8] = [i, i, i, i, i, i, i, i];
            schedule_task::spawn(1, 2, 0, 2, time + 100, data1).ok();
        }
        request_task::spawn(0x46).ok();

        //compare
    }

    #[task(shared=[sharedtaskid, data_from_can, local_tasklist], local=[], priority = 2)]
    fn schedule_task(
        ctx: schedule_task::Context,
        prio: u8,
        rec: u8,
        port: u8,
        cmd: u8,
        time: i32,
        data: [u8; 8],
    ) {
        //defmt::info!("task schedule");
        let mut sending = Vec::<[u8; 8], 32>::new();
        let mut local_tasklist = ctx.shared.local_tasklist;
        //create the new task command [prio, rec, port, cmd, time, time, time, time] - these are values for the new task
        //defmt::info!("sent schedule time {}", time);
        let prio: u8 = prio;
        let rec: u8 = rec;
        let port: u8 = port;
        let cmd: u8 = cmd;
        let time: i32 = time;
        let time1: u8 = (time >> 24) as u8;
        let time2: u8 = (time >> 16) as u8;
        let time3: u8 = (time >> 8) as u8;
        let time4: u8 = time as u8;
        let mut data = data;
        //defmt::info!("schedule time: {}, {}, {}, {}", time1, time2, time3, time4);
        // time.to_be_bytes()
        let header = [prio, rec, port, cmd, time1, time2, time3, time4];
        sending.push(header).ok();
        for _i in 0..30 {
            sending.push(data).ok();
        }

        //CAN header values [priority, receiver, port, command, sb, eb, frg_count]
        let priority: u8 = 1;
        let reciever: u8 = 1; //obc
        let port: u8 = 3; //fp
        let cmd: u8 = 2; //schedule
        loop {
            let err =
                can_send::spawn(priority, reciever, port, cmd, sending.clone(), true).is_err();
            if !err {
                break;
            }
        }

        let mut datatmp = Vec::<[u8; 8], 32>::new();
        let mut data_from_can = ctx.shared.data_from_can;
        let mut sharedtaskid = ctx.shared.sharedtaskid;
        //lock task untill it has received the reply from obc
        //defmt::info!("task is locked");
        let mut id: [u8; 8] = [0; 8];
        loop {
            data_from_can.lock(|c| {
                datatmp.clear();
                datatmp.extend(c.clone().into_iter())
            });
            if !datatmp.is_empty() {
                data_from_can.lock(|c| {
                    id[0] = c[0][6];
                    id[1] = c[0][7]
                }); //bytes [x,x,x,x,x,x,ID,ID] containing the id of the task saved as [ID,ID,x,x,x,x,x,x]
                    // copy over to compare variables
                sharedtaskid.lock(|c| c.push(id).ok()); //place id in task id list for further refrence
                data_from_can.lock(|c| c.clear()); //clear data ready for next receive
                break;
            }
        }
        sending.push([0; 8]).ok();
        let number = sending.len() - 1;
        for i in 0..(number - 1) {
            sending[number - i].rotate_right(2);
            sending[number - i][0] = sending[(number - i) - 1][6];
            sending[number - i][1] = sending[(number - i) - 1][7];
        }
        sending[1][0] = id[0];
        sending[1][1] = id[1];
        local_tasklist.lock(|c| c.push(sending).ok());

        //defmt::info!("schedule task has received confirmation");
    }

    #[task(shared=[sharedtaskid, data_from_can, local_tasklist], priority=2)]
    fn alter_task(
        ctx: alter_task::Context,
        task: usize,
        prio: u8,
        rec: u8,
        port: u8,
        cmd: u8,
        time: i32,
        data: [u8; 8],
    ) {
        defmt::info!("Alter Task");
        let mut sending = Vec::<[u8; 8], 32>::new();
        let mut local_tasklist = ctx.shared.local_tasklist;
        //create the new task command [prio, rec, port, cmd, time, time, time, time] - these are values for the new task
        let task_header: [u8; 8];
        let prio: u8 = prio;
        let rec: u8 = rec; //radio
        let port: u8 = port; //radio port (TBD)
        let cmd: u8 = cmd; //send command (TBD)
        let time: i32 = time; //time given from task
        let time1: u8 = (time >> 24) as u8;
        let time2: u8 = (time >> 16) as u8;
        let time3: u8 = (time >> 8) as u8;
        let time4: u8 = time as u8;
        let mut data = data;
        //defmt::info!("schedule time: {}, {}, {}, {}", time1, time2, time3, time4);
        task_header = [prio, rec, port, cmd, time1, time2, time3, time4];
        sending.push(task_header).ok();

        //adding task ID as [[x,x,x,x,x,x,x,x,],[ID,ID,0,0,0,0,0,0],...,[x,x,x,x,x,x,x,x]]
        let mut sharedtaskid = ctx.shared.sharedtaskid;
        let mut id = sharedtaskid.lock(|a| {
            a[task];
            a.remove(task)
        });
        for i in 0..6 {
            id[i + 2] = data[i];
        }
        sending.push(id).ok();
        local_tasklist.lock(|c| {
            c.push(sending.clone()).ok();
            c.remove(task)
        });
        //CAN header values [priority, receiver, port, command, sb, eb, frg_count]
        let priority: u8 = 1;
        let reciever: u8 = 1; //obc
        let port: u8 = 3; //fp
        let cmd: u8 = 3; //alter task
        can_send::spawn(priority, reciever, port, cmd, sending, true).ok();

        let mut datatmp = Vec::<[u8; 8], 32>::new();
        let mut data_from_can = ctx.shared.data_from_can;
        //lock task untill it has received the reply from obc

        loop {
            data_from_can.lock(|c| {
                datatmp.extend(c.clone().into_iter());
                c.clear()
            });
            if !datatmp.is_empty() {
                //defmt::info!("received alteration confirmation");
                let mut id: [u8; 8] = [0; 8]; //bytes [x,x,x,x,x,x,ID,ID] containing the id of the task saved as [ID,ID,x,x,x,x,x,x]
                id[0] = datatmp[0][6];
                id[1] = datatmp[0][7];
                sharedtaskid.lock(|c| c.push(id).ok()); //place id in task id list for further refrence
                data_from_can.lock(|c| c.clear()); //clear data ready for next receive
                break;
            }
        }
    }

    //request task list
    #[task(shared=[data_from_can, local_tasklist], priority=2)]
    fn request_task(mut ctx: request_task::Context, size: u8) {
        defmt::info!("request schedule");
        //the data here is irrelevant so an empty frame is sent
        let mut sending = Vec::<[u8; 8], 32>::new();
        let size = size;
        sending.push([0; 8]).ok();
        sending[0][0] = size;

        //CAN header values [priority, receiver, port, command, sb, eb, frg_count]
        let priority: u8 = 1;
        let reciever: u8 = 1; //obc
        let port: u8 = 3; //fp
        let cmd: u8 = 1; //request task
        can_send::spawn(priority, reciever, port, cmd, sending, true).ok();

        let mut datatmp = Vec::<[u8; 8], 32>::new();
        let mut vectorfull = Vec::<Vec<[u8; 8], 32>, 48>::new();
        let mut data_from_can = ctx.shared.data_from_can;
        //lock task untill it has received the reply from obc
        loop {
            let status = data_from_can.lock(|c| !c.is_empty());
            if status {
                data_from_can.lock(|c| {
                    datatmp.extend(c.clone().into_iter());
                    c.clear()
                });
                if datatmp[0][0] == 0x17 {
                    //defmt::info!("all tasks received");
                    break;
                }
                vectorfull.push(datatmp.clone()).ok();
                datatmp.clear();
            }
        }
        let mut local_tasklist = Vec::<Vec<[u8; 8], 32>, 48>::new();
        ctx.shared
            .local_tasklist
            .lock(|c| local_tasklist.extend(c.clone().into_iter()));
        //to see what is in the local list
        // for i in 0..local_tasklist.len() {
        //     for j in 0..local_tasklist[i].len() {
        //         defmt::info!("local message: {:#06X}", local_tasklist[i][j]);
        //     }
        // }

        //comparison
        let mut same = true;
        if local_tasklist.len() == vectorfull.len() {
            for i in 0..vectorfull.len() {
                if local_tasklist[i].len() == vectorfull[i].len() {
                    for j in 0..vectorfull[i].len() {
                        if local_tasklist[i][j] != vectorfull[i][j] {
                            same = false;
                        }
                    }
                } else {
                    same = false;
                }
            }
        } else {
            same = false;
        }
        defmt::println!("expected: {}", same);
    }

    #[task(shared=[sharedtaskid, data_from_can, local_tasklist], priority=2)]
    fn delete_task(ctx: delete_task::Context, task: usize) {
        defmt::info!("delete task");
        let mut sending = Vec::<[u8; 8], 32>::new();

        //adding task ID as [[ID,ID,0,0,0,0,0,0],...,[x,x,x,x,x,x,x,x]]
        let mut sharedtaskid = ctx.shared.sharedtaskid;
        let mut local_tasklist = ctx.shared.local_tasklist;
        let id = sharedtaskid.lock(|a| {
            a[task];
            a.remove(task)
        });
        local_tasklist.lock(|c| c.remove(task));

        sending.push(id).ok(); //sending the task ID for deletion
                               // for i in 0..sending.len() {
                               //     //defmt::info!("sending value: {}", sending[i]);
                               // }

        //CAN header values [priority, receiver, port, command, sb, eb, frg_count]
        let priority: u8 = 1;
        let reciever: u8 = 1; //obc
        let port: u8 = 3; //fp
        let cmd: u8 = 4; //delete task
        can_send::spawn(priority, reciever, port, cmd, sending, true).ok();

        let mut datatmp = Vec::<[u8; 8], 32>::new(); //temporary data
        let mut data_from_can = ctx.shared.data_from_can; //data from receive can
                                                          //lock task untill it has received the reply from obc
        loop {
            data_from_can.lock(|c| {
                datatmp.extend(c.clone().into_iter());
                c.clear()
            });
            if !datatmp.is_empty() {
                //defmt::info!("received a task deletion message");
                data_from_can.lock(|c| c.clear()); //clear data ready for next receive
                break;
            }
        }
    }

    #[task(priority = 5)]
    fn reply(_ctx: reply::Context) {
        let mut sending = Vec::<[u8; 8], 32>::new();
        sending.push([0x06, 0, 0, 0, 0, 0, 0, 0]).ok();
        let priority: u8 = 5;
        let reciever: u8 = 1; //obc
        let port: u8 = 3; //fp
        let cmd: u8 = 0; //reply
                         //defmt::info!("execution reply sent");
        can_send::spawn(priority, reciever, port, cmd, sending, true).ok();
    }
}
