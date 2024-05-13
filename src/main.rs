#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::dac::{Dac, ValueArray};
use embassy_stm32::dma::NoDma;
use embassy_stm32::pac::timer::vals::Mms;
use embassy_stm32::rcc::frequency;
use embassy_stm32::timer::low_level;
use embassy_stm32::{ peripherals, time::Hertz};
use defmt_rtt as _;
use panic_probe as _;

use micromath::F32Ext;

fn to_sine_wave(v: u8) -> u8 {
    if v >= 128 {
        // top half
        let r = 3.14 * ((v - 128) as f32 / 128.0);
        (r.sin() * 128.0 + 127.0) as u8
    } else {
        // bottom half
        let r = 3.14 + 3.14 * (v as f32 / 128.0);
        (r.sin() * 128.0 + 127.0) as u8
    }
}

fn calculate_array<const N: usize>() -> [u8; N] {
    let mut res = [0; N];
    let mut i = 0;
    while i < N {
        res[i] = to_sine_wave(i as u8);
        i += 1;
    }
    res
}

#[embassy_executor::main]
async fn main(_spawner: Spawner){
    info!("Hello world");

    let p = embassy_stm32::init(Default::default());

    info!("Initialising dac");
    let dma = p.DMA1_CH1;
    let (mut dac_ch1, mut dac_ch2) = Dac::new(p.DAC1, dma, NoDma, p.PA4, p.PA5).split();
    //let mut dac = DacCh1::new(p.DAC1, p.DMA1_CH3, p.PA4);
    let buf : &[u8; 128] = &calculate_array::<128>();
    dac_ch1.set_trigger(embassy_stm32::dac::TriggerSel::Tim6);
    dac_ch1.set_triggering(true);
    dac_ch1.enable();
    dac_ch2.disable();

    // initialising timer
    info!("Timer frequency: {}", frequency::<peripherals::TIM6>());
    let tim = low_level::Timer::new(p.TIM6);
    const FREQ: Hertz = Hertz::hz(10);
    let reload = (frequency::<peripherals::TIM6>().0 / FREQ.0) / buf.len() as u32;
    tim.regs_basic().arr().modify(|w| w.set_arr(reload as u16 - 1)); // set auto reload reg
    tim.regs_basic().cr2().modify(|w| w.set_mms(Mms::UPDATE));
    tim.regs_basic().cr1().modify(|w| {
        w.set_opm(false);
        w.set_cen(true);
    });

    info!("TIM frequency {}, target frequency {}, reload {}, reload as u16 {}, samples {}",
        frequency::<peripherals::TIM6>(),
        FREQ,
        reload,
        reload as u16,
        buf.len(),
    );

    loop {
        info!("Writing DAC");
        dac_ch1.write(ValueArray::Bit8(buf), false).await;
    }
}
