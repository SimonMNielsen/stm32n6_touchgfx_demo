//! TouchGFX application-specific STM32N6 clock configuration.

use embassy_stm32::rcc::{IcConfig, Icint, Icsel};
use embassy_stm32::Peripherals;

/// Initialize the board using the BSP defaults plus the system-bus clocks
/// required by this application's STM32N6/Embassy configuration.
pub fn init() -> Peripherals {
    let mut config = bsp_stm32n6570::clock::rcc_setup::n6570_clock_config();

    // IC2, IC6, and IC11 are the coupled STM32N6 system-bus clock group.
    // Embassy requires all three to be enabled before selecting IC2 as SYSCLK.
    let sys_ic = IcConfig {
        source: Icsel::Pll1,
        divider: Icint::Div4,
    };
    config.rcc.ic2 = Some(sys_ic);
    config.rcc.ic6 = Some(sys_ic);
    config.rcc.ic11 = Some(sys_ic);

    embassy_stm32::init(config)
}