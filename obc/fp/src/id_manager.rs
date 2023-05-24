/*
For the app module:
    use crate::id_manager::{test};

    extern "Rust" {
        #[task(shared = [flash, next_address_id])]
        fn test(_ctx: test::Context);
    }

    #[derive(defmt::Format)]
    pub enum FpConfig {
        TaskSize = 256,
        TaskNum = 48,
        StartAddress = 0x0,
    }

 */

#![no_std]
//Imports for ease of use.
use super::app;
use super::app::FpConfig as cfg;
use defmt::Format;
use dwt_systick_monotonic::ExtU32;
use flash::w25q128::Memory;
use rtic::Mutex;
use stm32f4xx_hal::spi::Instance;

//Unit enum to show FP task status:
#[derive(Format, Debug)]
enum TaskStatus {
    Empty,
    Scheduled,
    Executed,
    Invalid(u8), //If task status is invalid, this is the byte that caused it.
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    FPFull,
}

//Rtic task:
pub fn FP_task_id_manager(_ctx: app::FP_task_id_manager::Context) {
    //Declare our shared variables
    let mut flash = _ctx.shared.flash;
    let mut next_address = _ctx.shared.next_address_id;
    //Lock the variables:
    flash.lock(|f| {
        next_address.lock(|id| {
            *id = find_empty_task(f); //Update next address:
        })
    });
}

//Looks at the status byte in of the task:
fn determine_task_status(byte: u8) -> Result<TaskStatus, TaskStatus> {
    if (byte & 0xff) == 0xff {
        Ok(TaskStatus::Empty)
    } else if (byte & 0xf) == 0xf {
        Ok(TaskStatus::Scheduled)
    } else if (byte & 0x5) == 0x5 {
        Ok(TaskStatus::Executed)
    } else {
        Err(TaskStatus::Invalid(byte))
    } //Return error if not recognised.
}

//Return an address for a empty space in memory.
fn find_empty_task<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
) -> Result<u32, Error> {
    let index = 2; //Index of the status byte
    let mut executed_tasks: [u32; 48] = [0; 48]; //Array to store addresses of executed tasks
    let mut executed_tasks_index = 0; //Index of executed tasks array

    let end_address = cfg::StartAddress as u32 + (cfg::TaskSize as u32 * cfg::TaskNum as u32); //Calculate end address of FP
    let mut addr = cfg::StartAddress as u32 + index; //Address pointer, with index of status byte.
    let mut status = match determine_task_status(read_byte(flash, addr)) {
        Ok(status) => status,
        Err(e) => {
            defmt::panic!("Wrong byte! {}", e);
            e
        }
    }; //Return status of task.

    defmt::info!("Status of addr: {:x}, {}", addr, status); //Debugging
                                                            //Wait for a empty task or end of FP
    while !matches!(status, TaskStatus::Empty) && addr < end_address {
        //Log executed tasks
        if matches!(status, TaskStatus::Executed) {
            executed_tasks[executed_tasks_index] = addr - index;
            executed_tasks_index += 1;
        }
        addr += cfg::TaskSize as u32; //Go to next task
        status = determine_task_status(read_byte(flash, addr)).unwrap(); //Return status of task.
        defmt::info!("Status of addr: {:x}, {}", addr, status); //Debugging
    }
    //If we found a empty task, return the address.
    if addr < end_address {
        Ok(addr - index)
    } else {
        defmt::info!("No empty tasks found, making space"); //Debugging
        if executed_tasks_index > 0 {
            make_space_all(flash, &executed_tasks);
            Ok(executed_tasks[0]) //Give back the now empty address.
        } else {
            Err(Error::FPFull)
        } //If no executed tasks, return FP full error.
    }
}

//Removes the first executed task from flash
fn make_space<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
    executed_spaces: &[u32],
) {
    let mut data = [0u8; 4096]; //Buffer of sector size.
    let end_addr = executed_spaces[0] + cfg::TaskSize as u32; //First address space after task
    let start_addr = (executed_spaces[0] / 0x1000) * 0x1000; //Go to the start of the sector
    defmt::info!("Start addr: {:x}, end addr: {:x}", start_addr, end_addr); //Debugging
    let space_before_after = [executed_spaces[0] - start_addr, 4096 - end_addr]; //Determine area to save before and after task.
    defmt::info!(
        "Space before: {:x}, space after: {:x}",
        space_before_after[0],
        space_before_after[1]
    ); //Debugging

    //Read still valid sector content:
    flash.read(start_addr, space_before_after[0] as usize, &mut data);
    flash.read(
        end_addr,
        space_before_after[1] as usize,
        &mut data[space_before_after[0] as usize..],
    );

    //Erase sector:
    flash.delete(flash::w25q128::Delete::SectorErase, start_addr);

    //Put back data
    flash.write(start_addr, &data[..space_before_after[0] as usize]); //Write before task
    flash.write(
        end_addr,
        &data[space_before_after[0] as usize
            ..space_before_after[0] as usize + space_before_after[1] as usize],
    ) //Write after task
}
//Read single byte from flash
fn read_byte<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
    addr: u32,
) -> u8 {
    let mut byte = [0u8; 1];
    flash.read(addr, 1, &mut byte);
    byte[0]
}

//Removes all executed tasks from flash.
fn make_space_all<SPI: Instance, PINS, const P: char, const N: u8, MODE>(
    flash: &mut Memory<SPI, PINS, P, N, MODE>,
    executed_spaces: &[u32],
) {
    let mut data = [0u8; 4096]; //Buffer of sector size.
    let start_addr: u32 = executed_spaces[0] / 0x1000 * 0x1000; //Go to the start of the sector

    //Read still valid sector content:
    flash.read(start_addr, executed_spaces[0] as usize, &mut data); //Read until first index
    let mut i = 1;
    let mut index = executed_spaces[0];
    let mut lastaddress = executed_spaces[0];
    //All other tasks that are ready to be deleted has a address value over 0;
    while executed_spaces[i] > 0 && executed_spaces[i] < start_addr + 0xfff {
        //While we are still in the same sector
        let len = executed_spaces[i] - lastaddress - cfg::TaskSize as u32;
        flash.read(
            lastaddress + cfg::TaskSize as u32,
            len as usize,
            &mut data[index as usize..],
        ); //Concatenate to data buffer.
        index += len; //Update index
        lastaddress = executed_spaces[i];
        i += 1;
    }
    //Fetch the remainder of the flash:
    flash.read(
        lastaddress + cfg::TaskSize as u32,
        4096 - lastaddress as usize - cfg::TaskSize as usize,
        &mut data[index as usize..],
    );

    //Erase sector:
    flash.delete(flash::w25q128::Delete::SectorErase, start_addr);

    //Put back data
    flash.write(start_addr, &data[..executed_spaces[0] as usize]); //Write before first executed
    let mut next_address = executed_spaces[0];
    let mut index = executed_spaces[0];
    let mut i = 0;
    while executed_spaces[i + 1] > 0 && executed_spaces[i + 1] < (start_addr + 0xfff) || i == 0 {
        //Address and sector control.
        let next_index = index + executed_spaces[i + 1] - next_address - cfg::TaskSize as u32; //Determine data range.
        flash.write(
            executed_spaces[i] + cfg::TaskSize as u32,
            &data[index as usize..next_index as usize],
        ); //Write to flash, after executed task.
        next_address = executed_spaces[i + 1];
        index = next_index; //Update data index.
        i += 1; //Update task index.
    }
    if i + 1 < 16 {
        flash.write(
            next_address + cfg::TaskSize as u32,
            &data[index as usize..4095 - cfg::TaskSize as usize * (i + 1)],
        ); //Write after last executed task.
    }
}
