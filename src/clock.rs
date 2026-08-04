//! TouchGFX application-specific STM32N6 clock configuration.
//!
//! Wraps [`bsp_stm32n6570::clock::rcc_setup::n6570_clock_config`] with the
//! system-bus clock islands (IC2 + IC6 + IC11) that this app needs but the
//! BSP baseline intentionally leaves unset. Analogous to the
//! `common::init_peripherals` wrapper in the blazeface demo.

use embassy_stm32::rcc::{IcConfig, Icint, Icsel};
use embassy_stm32::Peripherals;

/// Initialize embassy with the BSP baseline clock tree plus IC2/IC6/IC11.
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