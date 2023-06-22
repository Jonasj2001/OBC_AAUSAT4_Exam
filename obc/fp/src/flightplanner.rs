pub mod flightplanner {

    use defmt::Format;
    use heapless::Vec;

    #[derive(Copy, Clone, Format)]
    pub struct FFArray {
        pub id: u32,
        pub execution_time: i32,
        pub priority: u8,
        pub dlc: u8,
    }

    #[derive(Clone)]
    pub struct FirstFive {
        pub content: Vec<FFArray, 5>,
    }

    impl FirstFive {
        pub fn new() -> Self {
            /*Creates an empty FF list*/
            let ff = Vec::<FFArray, 5>::new();
            FirstFive { content: ff }
        }

        pub fn add(&mut self, id: u32, execution_time: i32, priority: u8, dlc: u8) {
            /*Basicly just push*/
            self.content
                .push(FFArray {
                    id,
                    execution_time,
                    priority,
                    dlc,
                })
                .ok();
        }

        pub fn update(&mut self, ff: Vec<FFArray, 5>) {
            /*Short function to delete and update the First Five*/
            self.content.clear();
            self.content.extend(ff.into_iter());
        }
    }

    pub fn sort_to_ff<const N: usize>(inserted_list: &Vec<FFArray, N>) -> Vec<FFArray, 5> {
        //Creates a new list for cloning
        let mut list = Vec::<FFArray, N>::new();
        list.extend(sort_full_list(inserted_list).into_iter());

        //Throws away elements if the list is longer than five elements
        //Nessecary as extend panics if the list is longer than the targets size
        if list.len() > 5 {
            defmt::debug!("Popping {} elements!", list.len() - 5);
            list.truncate(5);
        }
        let mut output_list = FirstFive::new();
        output_list.content.extend(list.into_iter());
        output_list.content
    }

    pub fn sort_full_list<const N: usize>(inserted_list: &Vec<FFArray, N>) -> Vec<FFArray, N> {
        //Creates a new list for cloning
        let mut list = Vec::<FFArray, N>::new();
        list.clone_from(inserted_list);

        //Checks for content to sort - list MUST be at least two long for it to make sense
        if list.len() > 1 {
            //Sorts the list after execution time - Be aware that priority might get mixed
            list.sort_unstable_by_key(|l| l.execution_time);
            //Task for comparision
            let mut prev_task: FFArray = list[0];
            let mut repeat = true;
            while repeat {
                /*Checks the list for execution time and priority.
                If execution time is the same, and priority is higher, swap the tasks.
                Continues until the whole list has been worked through, and nothing was swapped*/
                repeat = false;
                for task in 1..list.len() {
                    if (list[task].execution_time == prev_task.execution_time)
                        && (list[task].priority > prev_task.priority)
                    {
                        list.swap(task, task - 1);
                        repeat = true;
                    }
                    //Sets the previous task as the current one.
                    prev_task = list[task];
                }
                //Restarts the list
                prev_task = list[0];
            }
        }
        list
    }

    pub fn print_ff<const N: usize>(ff: &Vec<FFArray, N>) {
        defmt::debug!("Printtime");
        for element in 0..ff.len() {
            defmt::debug!(
                "Element nr: {} - ID:{}, EXE:{}, PRIO:{}, DLC:{}",
                element,
                ff[element].id,
                ff[element].execution_time,
                ff[element].priority,
                ff[element].dlc
            );
        }
    }

    pub fn compile_task(data: &Vec<[u8; 8], 32>, debug: bool) -> [u8; 256] {
        let mut task: [u8; 256] = [0; 256];

        //Id is imported - moved from [00000PPP][0000RRRR][0000ppp][CCCCCCCC] => [00000000][PPPRRRRp][ppCCCCCC][CCEEEEEE]
        //Where first P is Priority, R is Reciever, p is port, C is command and E is Executed. - Only last three bytes are relevant for the Task.
        //Note: Priority is pushed to the start, for easier sorting.
        let can_id: [u8; 4] = {
            let mut can_id: u32 = (data[0][0] & 0b00000111) as u32;
            can_id = (can_id << 4) | (data[0][1] & 0b00001111) as u32;
            can_id = (can_id << 3) | (data[0][2] & 0b00000111) as u32;
            can_id = (can_id << 8) | data[0][3] as u32;
            ((can_id << 6) | 0b00001111).to_be_bytes()
        };

        //Data Lenght Code is established as the received data, minus 1 (that is the CAN_ID and the execution time)
        //Program should throw an error if DLC is larger than 30.
        //@TODO: Determine maximum lenght
        let dlc = (data.len()) as u8;

        //Empty array for execution time
        let mut execution_time: [u8; 4] = [0; 4];

        //CAN_ID is placed in the first three spots of the task
        for x in 0..=2 {
            task[x as usize] = can_id[x + 1];
        }

        //Execution time is placed from [3] to [6]
        for x in 0..=3 {
            task[x + 3 as usize] = data[0][x + 4];
            execution_time[x] = data[0][x + 4];
        }

        //task [7] is the DLC - Lets us know the amount of data to read.
        task[7] = dlc;

        //Rest of the data is filled into the correct spots - There are 8 bytes per can frame, and up to DLC number of frames that are to be logged
        for frame_nr in 1..dlc {
            for part_nr in 0..8 {
                task[(frame_nr * 8 + part_nr) as usize] =
                    data[(frame_nr) as usize][part_nr as usize];
            }
        }

        if debug {
            defmt::debug!("Recieved ID: {:#010b}", can_id);
            defmt::debug!("Data is {} long", dlc);
            defmt::debug!("Data includes: {:#04X}", data[1..dlc as usize]);
            defmt::debug!("current task:{:#04X}", task);
        }
        //Returns task
        task
    }

    pub fn compare_tasks(task1: &[u8; 256], task2: &[u8; 256]) -> bool {
        //Compares two tasks, and returns true if they are the same
        let mut same = true;
        for x in 0..256 {
            if task1[x] != task2[x] {
                same = false;
            }
        }
        same
    }

    pub fn is_execute_ready(byte: u8) -> bool {
        let scheduled: bool = 0 == (byte & 0b00110000);
        let executed: bool = 0b00000101 == (byte & 0b00111111);
        //Only interested in scheduled tasks, not executed ones
        if scheduled && !executed {
            true
        } else {
            false
        }
    }

    pub fn decompile_task(task: &mut [u8; 256], address: u32) -> Vec<[u8; 8], 32> {
        defmt::debug!("Decompiling task: {}", address);
        let mut dlc = task[7] as usize;
        //[PPPRRRRp][ppCCCCCC][CCEEEEEE] => [00000PPP][0000RRRR][0000ppp][CCCCCCCC]
        let mut data_vec = Vec::<[u8; 8], 32>::new();
        //Extract data from the compact version
        let prio = task[0] >> 5;
        let rec = (task[0] >> 1) & 0b00001111;
        let port = (task[0] & 0b00000001) << 2 | (task[1] >> 6);
        let cmd = (task[1] << 2) | (task[2] >> 6);

        //Makes sure that we do not read more than possible
        let mut max = dlc * 8;
        if max == 255 {
            max = 253;
        }

        //Shifts data two spaces to the right
        for i in (3..7).rev() {
            //defmt::debug!("time b{} with b{}", i + 2, i,);
            task[i + 1] = task[i];
        }

        //Shifts data two spaces to the right
        for i in (9..max).rev() {
            //defmt::debug!("replace b{} with b{}", i + 2, i,);
            task[i + 2] = task[i];
        }

        task[9] = address as u8;
        task[8] = (address >> 8) as u8;
        task[3] = cmd;
        task[2] = port;
        task[1] = rec;
        task[0] = prio;

        defmt::debug!("Task: {}", task);

        if dlc == 32 {
            dlc = 31;
        }

        for i in 0..(dlc + 1) {
            let s_b = (i) * 8;
            let e_b = (i + 1) * 8;
            data_vec
                .push(task[s_b..e_b].try_into().expect("Not a real size"))
                .ok();
        }
        data_vec
    }
}
