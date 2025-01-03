#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};

use defmt::*;
use embassy_executor::Executor;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
static CHANNEL: Channel<CriticalSectionRawMutex, LedState, 1> = Channel::new();
static LED_INDEX: AtomicU32 = AtomicU32::new(0); // 0..7

enum LedState {
    On,
    Off,
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    let leds = [
        Output::new(p.PIN_4, Level::High),
        Output::new(p.PIN_5, Level::High),
        Output::new(p.PIN_6, Level::High),
        Output::new(p.PIN_7, Level::High),
        Output::new(p.PIN_8, Level::High),
        Output::new(p.PIN_9, Level::High),
        Output::new(p.PIN_10, Level::High),
        Output::new(p.PIN_11, Level::High),
    ];

    let _button1 = Input::new(p.PIN_3, Pull::Up);
    let _button2 = Input::new(p.PIN_12, Pull::Up);

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                unwrap!(spawner.spawn(core1_task(leds)));
                unwrap!(spawner.spawn(led_index_cycle_task()));
            });
        },
    );
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| unwrap!(spawner.spawn(core0_task())));
}

#[embassy_executor::task]
async fn core0_task() {
    info!("Hello from core 0");
    loop {
        CHANNEL.send(LedState::On).await;
        Timer::after_millis(500).await;
        CHANNEL.send(LedState::Off).await;
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::task]
async fn led_index_cycle_task() {
    loop {
        Timer::after_millis(3000).await;
        let index = LED_INDEX.load(Ordering::Relaxed);
        LED_INDEX.store((index + 1) % 8, Ordering::Relaxed);
    }
}

#[embassy_executor::task]
async fn core1_task(mut leds: [Output<'static>; 8]) {
    info!("Hello from core 1");
    loop {
        match CHANNEL.receive().await {
            LedState::On => {
                for led in leds.iter_mut() {
                    led.set_high();
                }
            }

            LedState::Off => {
                let index: u32 = LED_INDEX.load(Ordering::Relaxed);
                leds[index as usize].set_low();
            }
        }
    }
}
