----------------------------------------------------------------------------------
-- Company: 
-- Engineer: 
-- 
-- Create Date: 28.04.2023 09:49:02
-- Design Name: 
-- Module Name: StateMachine - Behavioral
-- Project Name: 
-- Target Devices: 
-- Tool Versions: 
-- Description: 
-- 
-- Dependencies: 
-- 
-- Revision:
-- Revision 0.01 - File Created
-- Additional Comments:
-- 
----------------------------------------------------------------------------------


library IEEE;
use IEEE.STD_LOGIC_1164.ALL;
use IEEE.numeric_std.ALL;
use IEEE.STD_LOGIC_Unsigned.ALL;

-- Uncomment the following library declaration if using
-- arithmetic functions with Signed or Unsigned values
--use IEEE.NUMERIC_STD.ALL;

-- Uncomment the following library declaration if instantiating
-- any Xilinx leaf cells in this code.
--library UNISIM;
--use UNISIM.VComponents.all;

entity StateMachine is
  Port (
    CLOK            : in std_logic;
    BitstreamIn     : in std_logic;
    CS              : in std_logic;
    SingalOut       : out std_logic;
    Q               : out std_logic;
    InfoOutput      : out std_logic_vector(1 downto 0);
    
    SignalStoredOut : out std_logic;
    signalStoredFlag: out std_logic;
    signalFSMFlag   : out std_logic;
    SignalClockIn   : in std_logic;
    SignalGetInfo   : in std_logic;
    reg0   : out std_logic;
    reg1   : out std_logic
   );
end StateMachine;

architecture Behavioral of StateMachine is


 signal tmp: std_logic_vector(55 downto 0) := x"00000000000000"; --Shiftregister for Callsign and FSM maker
 signal CLR: std_logic := '1'; --initialization of counter
 signal FSM: std_logic := '0'; -- FSM maker for incomming data
  
 signal countTmp: unsigned(10 downto 0); --11bit counter
 
 signal storeTmp0: unsigned(1999 downto 0); --Register0(buffer0) for incomming data
 signal storeTmp1: unsigned(1999 downto 0); --Register1(buffer1) for incommming data

 shared variable signalStoreCounterLongReg0 : integer := 1; --used for sending out data for long FSM for buffer0
 shared variable signalStoreCounterShortReg0 : integer := 1; --used for sending out data for short FSM for buffer0
 
 shared variable signalStoreCounterLongReg1 : integer := 1; --used for sending out data for long FSM for buffer1
 shared variable signalStoreCounterShortReg1 : integer := 1; --used for sending out data for short FSM for buffer1
 
 shared variable Register0sent: boolean := false; --used to check if data has been read from buffer0
 shared variable Register1sent: boolean := false; --used to check if data has been read from buffer1
 
 signal Register0sentGet: std_logic := '1'; --used to mark new data in buffer0
 signal Register1sentGet: std_logic := '1'; --used to mark new data in buffer1
 
 shared variable WriteRegister  : integer := 0; --counts what register needs to be wrote to
 shared variable ReadRegister   : integer := 0; --checks what register that needs to be read
 
 shared variable StoredFlag0    : integer := 0; --when buffer0 is full a flag is set here
 shared variable StoredFlag1    : integer := 0; --when buffer1 is full a flag is set here

 signal FSM0: std_logic := '0'; --saves what FSM buffer0 has
 signal FSM1: std_logic := '0'; --saves what FSM buffer1 has 
 
 signal FMSoutput: std_logic := '0'; --The outputtet FSM for the OBC
 
 signal WR0Write : std_logic := '1'; --Used to check if new data can be written in buffer0
 signal WR0Read : std_logic := '1'; --Used to check if data has been read from buffer0
 signal WR1Write : std_logic := '1';--Used to check if new data can be written in buffer1
 signal WR1Read : std_logic := '1'; --Used to check if data has been read from buffer1
 
 signal Flag :std_logic := '1';
 
begin
     reg0 <= WR0Read; --pinout to see the state of WR0Read, at every clock cycle
     --reg1 <= WR1Read; --pinout to see the state of WR1Read, at every clock cycle
     
    
    --process is entered every time a change in variable SignalClockIn, WR0Write or WR1Write happens 
    process(SignalClockIn, Flag)  
      begin  
       --The following code is executed when the SignalClockIn is on rising edge.
       --SignalStoredOut <='0';
    if ((SignalClockIn'event and SignalClockIn = '0')) then 
  
      --resetes the pins WR0Read and WR1Read to 0 every time the the buffers have been read.
      if WR0Read = '1' then
       signalStoreCounterLongReg0 := 1;
       signalStoreCounterShortReg0 := 1;
       Register0sent := false;
      end if;
      if WR1Read = '1' then
       signalStoreCounterLongReg1 := 1;
       signalStoreCounterShortReg1 := 1;
       Register1sent := false;
      end if;
        --WR0Read <= '1';
                if ReadRegister = 0 then --test if its register 0 we want to read.
                    if FMSoutput = '1' then --test if its short callsign we are sending.
                        SignalStoredOut <= storeTmp0(1024 - (signalStoreCounterShortreg0)); --output the stored data
                        signalStoreCounterShortReg0 := signalStoreCounterShortreg0 + 1; --count up to send the next bit stored
                    elsif FMSoutput = '0' then --if not shot, test if its long FSM
                        SignalStoredOut <= storeTmp0(2000 - (signalStoreCounterLongReg0)); --output the stored data
                        signalStoreCounterLongReg0 := signalStoreCounterLongReg0 + 1; --count up to send the next bit stored
                    end if;
                    
                --test if all data(Long FSM) have been send, if yes,  count up wich register that needs to be read next.    
                if signalStoreCounterLongReg0 = (2001) and FMSoutput = '0' then 
                    Register0sent := true;
                    ReadRegister := ReadRegister + 1;
                    if Readregister = 2 then 
                        ReadRegister := 0;   
                    end if;
                end if;
                
                --test if all data(short FSM) have been send, if yes,  count up wich register that needs to be read next.
                if signalStoreCounterShortReg0 = (1025) and FMSoutput = '1' then 
                    Register0sent := true;
                    ReadRegister := ReadRegister + 1;
                    if Readregister = 2 then 
                        ReadRegister := 0;   
                    end if;   
                end if;
            end if;    
            
            
           --here alle test are the same as the abovecode, the only differens is that its done with buffer1(register1)
           if ReadRegister = 1 then
                    if FMSoutput = '1' then
                        SignalStoredOut <= storeTmp1(1024 - (signalStoreCounterShortReg1)); --output the stored data :o --her er fejlen.
                        signalStoreCounterShortReg1 := signalStoreCounterShortReg1 + 1;
                    elsif FMSoutput = '0' then
                        SignalStoredOut <= storeTmp1(2000 - (signalStoreCounterLongReg1)); --output the stored data :o --her er fejlen.
                        signalStoreCounterLongReg1 := signalStoreCounterLongReg1 + 1;
                end if; 
                if signalStoreCounterLongReg1 = (2001) and FMSoutput = '0' then
                    Register1sent := true;
                    
                        ReadRegister := ReadRegister + 1;
                    if Readregister = 2 then 
                        ReadRegister := 0;   
                    end if;
                end if;
                    if signalStoreCounterShortReg1 = (1025) and FMSoutput = '1' then 
                        Register1sent := true;
                        ReadRegister := ReadRegister + 1;
                        if Readregister = 2 then 
                            ReadRegister := 0;   
                        end if;
                    end if;
                end if;
            end if;
--    if (Flag = '1') then
--        reg1 <= '1';
--        SignalStoredOut <= '0';
--            if ReadRegister = 0 then --test if its register 0 we want to read.
--                    if FMSoutput = '1' then --test if its short callsign we are sending.
--                        SignalStoredOut <= '0'; --output the stored data
--                    elsif FMSoutput = '0' then --if not shot, test if its long FSM
--                        SignalStoredOut <= '0'; --output the stored data
--                    end if;
--            end if; 
--            if ReadRegister = 1 then
--                    if FMSoutput = '1' then
--                        SignalStoredOut <= '0'; --output the stored data :o --her er fejlen.
--                    elsif FMSoutput = '0' then
--                        SignalStoredOut <= '0'; --output the stored data :o --her er fejlen.
--                end if; 
--            end if;              
--       end if;                 
    end process;

SignalStoredFlag <= Flag;
Flag <= '1' when (WR0Write = '0' and WR0Read = '0') or (WR1Write = '0' and WR1Read = '0') else '0';
signalFSMFlag <= FMSoutput;

--signalStoredFlag <= WR0Write; --output if something is stored in one of the buffers
--signalFSMFlag <= WR1Write; --outputs the relevant FSM for the current stored data.

--reg0 <= WR0Read;
--reg1 <= WR1Read;

--A multiplex statment that sets WR0Read HIGH when the data has been read and LOW if WR0Writ is low.
WR0Read <= '1' when ((signalStoreCounterLongReg0 = (2001)) or (signalStoreCounterShortReg0 = (1025))) and (Register0sent = true) else
           '0' when WR0Write = '0';

--A multiplex statment that sets WR1Read HIGH when the data has been read and LOW if WR1Writ is low.           
WR1Read <= '1' when ((signalStoreCounterLongReg1 = (2001)) or (signalStoreCounterShortReg1 = (1025))) and (Register1sent = true) else
           '0' when WR1Write = '0';

--sets the output of the WR0Write to HIGH if both the WR0Read and Register0sentGet is set to true.
--Register0sentGet is only set HIGH when the callsign is found, it will be LOW when ever the data has been stored.
WR0Write <= WR0Read and Register0sentGet;

--sets the output of the WR1Write to HIGH if both the WR1Read and Register1sentGet is set to true.
--Register1sentGet is only set HIGH when the callsign is found, it will be LOW when ever the data has been stored.
WR1Write <= WR1Read and Register1sentGet;

    --this is the process that handels the incomming radio transmission
    --looking for call sign and stores data in buffer.
    process(CLOK) 
        begin   
        
        if (CLOK'event and CLOK='1') then --Rising edge
        
            if(CS = '0') then --CS low for SPI Reasons. 
                for i in 0 to 54 loop   --Shift register.
                    tmp(i+1) <= tmp(i);
                end loop;
                tmp(0) <= BitstreamIn;               
            end if; --CS end.
            
           --Counter:
            if (CLR='1') then --resets counter             
                countTmp <= "00000000000";
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
                --SignalStoredFlag <= '0';
            else
                Q <= '1';
                SingalOut <= BitstreamIn; --send bitstream out (to check if data was recived)
                
                if WriteRegister = 0 then -- test what register that will be written to
                    FSM0 <= FSM;    
                  for i in 0 to 1998 loop   --Shift register for first buffer.
                        storeTmp0(i+1) <= storeTmp0(i);
                    end loop;          
                    storeTmp0(0) <= BitstreamIn; --Skriv data ind i bufferen
                 end if; 
                 
                if WriteRegister = 1 then
                   -- WR1Write <= '0';
                    FSM1 <= FSM;
                  for i in 0 to 1998 loop   --Shift register. 
                        storeTmp1(i+1) <= storeTmp1(i);
                    end loop;          
                    storeTmp1(0) <= BitstreamIn; --Skriv data ind i bufferen                
                end if;
                countTmp <= countTmp + 1; --count up 1 each clk.
            end if; --counter
            
            --11111010000 2000 i binary
            --test om counter er ramt XXX optælling.
            if (countTmp = "10000000000" and FSM = '1')then
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
                if ReadRegister = 0 then 
                    FMSoutput <= FSM0;
                    else 
                    FMSoutput <= FSM1;
                end if;
                             
                if WriteRegister = 0 then
                    StoredFlag0 := 1;
                    Register0sentGet <= '0';
                   -- WR0Write <= '0';
                elsif WriteRegister = 1 then
                    Register1sentGet <= '0';
                   -- WR1Write <= '0';
                    StoredFlag1 := 1;
                end if;
                
                WriteRegister := WriteRegister + 1;
                if WriteRegister = 2 then
                    WriteRegister := 0;
                end if;
                 
                CLR <= '1';  
            else if (countTmp = "11111010000" and FSM = '0') then
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
                if ReadRegister = 0 then 
                   -- signalFSMFlag <= FSM0;
                    FMSoutput <= FSM0;
                else 
                   -- signalFSMFlag <= FSM1;
                    FMSoutput <= FSM1;
                end if;
                                
                if WriteRegister = 0 then
                    StoredFlag0 := 1;
                    Register0sentGet <= '0';
                   -- WR0Write <= '0';
                elsif WriteRegister = 1 then
                    Register1sentGet <= '0';
                    --WR1Write <= '0';
                    StoredFlag1 := 1;
                end if;
                
                WriteRegister := WriteRegister + 1;
                if WriteRegister = 2 then
                    WriteRegister := 0;
                end if;
                
                CLR <= '1';
                end if; --end 2000 check 
            end if;--end 1024 count check                   
        end if; --clock(rising egde) end
        
        
        --find ud af om callsign er iorden
        --Short
        if (tmp = x"A64F5A36435542" and CLR = '1' and (WR0Write = '1' or WR1Write = '1')) then   
            FSM <= '1';
            InfoOutput(1) <= '0';
            InfoOutput(0) <= '1';
            Register0sentGet <= '1';
            Register1sentGet <= '1';
            CLR <= '0';
            
            --Long Register0sentGet = true or Register1sentGet = true
           else if (tmp = x"594F5A36435542" and CLR = '1' and (WR0Write = '1' or WR1Write = '1')) then
            FSM <= '0';
            InfoOutput(0) <= '0';
            InfoOutput(1) <= '1';
            Register0sentGet <= '1';
            Register1sentGet <= '1';
            CLR <= '0';
           end if;--test
            
           if (CLR = '1')then
            InfoOutput(0) <= '0';
            InfoOutput(1) <= '0';
           end if;--clr
        end if;--rising egde
end process; --clock process end.    
end Behavioral;
