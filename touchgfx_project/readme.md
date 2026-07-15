# STM32N6570-DK-FreeRTOS TBS

## Table of Contents
1. [Configuration](#configuration)
2. [Building and Flashing](#building-and-flashing)
3. [Debugging](#debugging)

## Configuration

This TBS is created as a **Secure domain only** project in STM32CubeMX. The project type cannot be changed after project creation.

**MDK-ARM** is not supported in this release.

The default IDE is set to STM32CubeIDE. To change the IDE, open the STM32N6570-DK.ioc with STM32CubeMX and select from the supported Toolchains (EWARM and STM32CubeIDE).

This TBS is configured for 800 x 480 pixels 16 bpp screen resolution with double framebuffers in internal RAM.

The project consists of a First Stage BootLoader (FSBL) and a secure application with ThreadX.

The default execution strategy is Load and Run (LRUN), which can be changed to Execute in Place (XIP) by the user following the steps outlined in this file if desired. Note that Execute In Place is not currently supported if using ST Arm Clang.

## Building and Flashing

For the STM32CubeIDE and IAR projects, there is a configuration for both an FSBL and a secure application
The FSBL copies the application to internal RAM before executing it.

For Run Target, clicking the "Run Target" button in TouchGFX Designer builds both the FSBL and Application, then signs and uploads them to the board without requiring further user input. Set the switches on the board according to the procedure for flashing described below.

To build using an IDE:
1. Generate code in TouchGFX Designer.
2. Build both the FSBL and the application from the IDE.

To build using CMake:
1. Generate code in TouchGFX Designer.
2. Configure and build your preferred preset from the project root.

To assist with signing and flashing the binaries, helper scripts are provided in the "Flash Scripts" folder. Flashing the board requires STM32CubeProgrammer installed in the default location which can be downloaded from st.com.

### Flashing Procedure:

1. **Preparation:**
   - Set the switch labeled BOOT1 (the bottom one) is in the rightmost position and reset the board. This will put the board in programming mode, and the screen will remain black.
   - If the flashing fails, it is sometimes necessary to move the switch labeled BOOT0 (the upper one) to the rightmost position as well.  
   - Scripts (.bat files) for flashing your application are provided in the *Flash Scripts/* folder of your project. To use them, simply double-click on the appropriate script.
 
3. **Flashing:**
   - Scripts are provided for both IAR and STM32CubeIDE, which also suppports both the Debug and the Release configurations. There are four scripts:
     1. One to load the FSBL.
     2. One to load only the application.
     3. One to load only the assets.
     4. A script that executes all of them in sequence.
   - If no changes have been made to the assets, a lot of time can be saved by only flashing the application.

4. **Post-Flashing:**
   - After flashing has finished, move the boot switch to the leftmost position. The application will start once the reset button is clicked or the USB cable is reconnected.

## Debugging

It is not possible to debug the application in the usual way. Instead it can be done in the following two ways:

1. **Connecting to running target:**
   - Flash the entire program (FSBL, Appli and assets) like described above. 
    - If using CubeIDE:
      - Launch a debug session with the configuration named STM32N6570-DK-FreeRTOS_Attach_To_Running.
   - If using IAR:
      - Go to Project -> Attach to Running Target in the menu with the application configuration open.

   At this point, breakpoints and stepping are possible as usual.

   A loop is available at the beginning of the main() function for use if the board crashes before it is possible to attach the debugger: 
   - Compile the project with the debugFlag variable set to 1
   - Flash, run and connect the debugger. 
   - The program will be caught in the loop. Pause execution and change the value of debugFlag with the debugger tool.
   - Breakpoints and single stepping can be used to locate the error.

2. **Debugging from FSBL:**
   The TBS supports continuing into the secure application when debugging the FSBL. This makes it possible to set breakpoints in the initialization code instead of using the catch loop.
   - Compile and flash as usual using the scripts.
   - If using CubeIDE:
      - Launch a debug session with the configuration named STM32N6570-DK-FreeRTOS_FSBL_Appli_Debug
   - If using IAR:
      - Set the FSBL sub-project as active project
      - Click "Download and Debug"
   - The IDE will download the FSBL to RAM, loading symbols for both FSBL and Appli. The debugger is stopped at the entry point of the FSBL, and breakpoints can be set as needed in both the FSBL and the secure application.

   If the program crashes to a state where it is not possible to connect the debugger, the following steps should resolve that:
   - Disconnect the board
   - Set the BOOT1 switch to the rightmost position
   - Connect the board
   - Set the BOOT1 switch to the leftmost position
   - Start the debug procedure as above

## Changing execution strategy

1. **Change strategy in STM32CubeMX:**
   - Navigate to Middleware and Software Packs -> EXTMEM_MANAGER
   - Change Selection of the boot system to Execute In Place. The correct values are preset in the .ioc file
   - Generate code
   This will result in a new FSBL being generated, which does not copy the code to RAM and has the entry point of the secure application in external flash.

2. **Change linker script in IDE:**
   Linker scripts that link the application for XIP are provided for both STM32CubeIDE and IAR
   - If using CubeIDE:
      - Right click the Appli project and select Properties
      - Navigate to C/C++ Build -> Settings
      - Under MCU/MPU G++ Linker, select General
      - Change the reference to the linker script from ```${workspace_loc:/${ProjName}/STM32N657XX_LRUN.ld}``` to ```${workspace_loc:/${ProjName}/STM32N657XX_XIP.ld}```
   - If using 'Run Target' in TouchGFX Designer:
      - Open 'makefile_appli' located in the gcc folder
      - Navigate to the line that contains ```STM32N657XX_LRUN.ld```
      - Replace ```STM32N657XX_LRUN.ld``` with ```STM32N657XX_XIP.ld```
   - If using CMake:
      - Open the 'CMakeLists.txt' located in the Appli folder
      - Navigate to the line that contains ```set(STM32_LINKER_SCRIPT STM32N657XX_LRUN.ld)```
      - Replace ```STM32N657XX_LRUN.ld``` with ```STM32N657XX_XIP.ld```
   - If using IAR:
      - Right click the Appli project and select Options
      - Navigate to Linker -> Config
      - Change the reference to the linker script from ```$PROJ_DIR$/STM32N657XX_LRUN.icf``` to ```$PROJ_DIR$/STM32N657XX_XIP.icf```


## Performance Testing

Performance testing can be done using the GPIO pins designated with the following signals in the Spare GPIO connector on the underside of the board:

- **VSYNC_FREQ**  - PF6 (CN11 pin 1)
- **RENDER_TIME** - PD5 (CN11 pin 2)
- **FRAME_RATE**  - PD0 (CN11 pin 3)
- **MCU_ACTIVE**  - PE9 (CN11 pin 4)