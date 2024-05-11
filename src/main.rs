#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::dac::{DacCh1, ValueArray};
use embassy_time::Timer;
use embassy_stm32::i2c::I2c;
use embassy_stm32::gpio::{ Level, Output, Speed };
use embassy_stm32::{ bind_interrupts, i2c, peripherals, time::Hertz};
use si5351;
use si5351::{ Si5351, Si5351Device };
use defmt_rtt as _;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

enum Direction {
    Up,
    Down,
}

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

#[embassy_executor::main]
async fn main(_spawner: Spawner){
    info!("Hello world");

    let p = embassy_stm32::init(Default::default());
    let bus = p.I2C1;
    let sda = p.PB7;
    let scl = p.PA15;
    let mut led = Output::new(p.PC6, Level::High, Speed::Low);
    let mut dac = DacCh1::new(p.DAC1, p.DMA1_CH3, p.PA4);
    let mut buf = [0; 255];
    for v in 0..255 {
        buf[v] = to_sine_wave(v.try_into().unwrap());
    }

    let i2c = I2c::new(
        bus, 
        scl,
        sda,
        Irqs,
        p.DMA1_CH1,
        p.DMA1_CH2,
        Hertz(100_000),
        Default::default(),
    );

    let mut clock = Si5351Device::new(i2c, false, 25_000_000);
    clock.init(si5351::CrystalLoad::_10).expect("clock init failed");

    let dac_stuff = async {
        dac.write(ValueArray::Bit8(&buf), true).await;
    };

    let pll_stuff = async {
        let mut dir = Direction::Up;
        let mut freq = 100_000;
      
        loop {
            led.toggle();
            info!("Setting frequency to {}", freq);
            clock.set_frequency(si5351::PLL::A, si5351::ClockOutput::Clk0, freq).expect("set frequency failed");
            let status = clock.read_device_status().expect("Failed to read status");
            info!("Status: {:#b}", status.bits());
    
            match freq {
                100_000 => dir = Direction::Up,
                200_000 => dir = Direction::Down,
                _ => (),
            };
            match dir {
                Direction::Up => freq += 10_000,
                Direction::Down => freq -= 10_000,
            };
    
            Timer::after_millis(1000).await;
        }
    };

    join(pll_stuff, dac_stuff).await;
}
