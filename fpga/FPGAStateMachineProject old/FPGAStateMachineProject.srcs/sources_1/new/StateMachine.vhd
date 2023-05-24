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
    InfoOutput      : out std_logic_vector(2 downto 0)
   );
end StateMachine;

architecture Behavioral of StateMachine is
 signal tmp: std_logic_vector(55 downto 0) := x"00000000000000";
 signal CLR: std_logic := '1';
 signal FSM: std_logic_vector(1 downto 0) := b"00";
 
 
 signal countTmp: unsigned(10 downto 0);
 
begin
    
    process(CLOK)
        begin
        if (CLoK'event and CLoK='1') then
            if(CS = '0') then
                for i in 0 to 54 loop
                    tmp(i+1) <= tmp(i);
                end loop;
                tmp(0) <= BitstreamIn;
            end if;
            
           --Counter:
            if (CLR='1') then
                countTmp <= "00000000000";
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
            else
                Q <= '1';
                countTmp <= countTmp + 1; --count up 1 each clk.
                SingalOut <= BitstreamIn; --send bitstream out.
            end if;
            
            --11111010000 2000 i binary
            --test om counter er ramt XXX optælling.
            if (countTmp = "10000000000" and FSM(0) = '1'  and FSM(1) = '0')then
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
                CLR <= '1';
                --InfoOutput(0) <= '0';
                
                
            else if (countTmp = "11111010000" and FSM(0) = '0' and FSM(1) = '1') then
                Q <= '0';--Q viser om vi har ramt det vi skulle tælle til.
                CLR <= '1';
                --InfoOutput(1) <= '0';
                end if;  
            end if;
            
            --find ud af om callsign er iorden
                   
        end if; --clock(rising egde) end
        
        --Short
        if (tmp = x"A64F5A36435542" and CLR = '1') then
            CLR <= '0';
            FSM(0) <= '1';
            InfoOutput(2) <= FSM(0);
            FSM(1) <= '0';
            InfoOutput(0) <= '1';
            
            --Long
           else if (tmp = x"594F5A36435542" and CLR = '1') then
            CLR <= '0';
            FSM(0) <= '0';
            FSM(1) <= '1';
            InfoOutput(1) <= FSM(1);
           end if;
            
           if (CLR = '1')then
           InfoOutput(0) <= '0';
           InfoOutput(1) <= '0';
           end if;
        end if;       
    end process; --clock process end.


end Behavioral;
