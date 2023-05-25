#![no_std]
#![no_main]

/// Alias for the HAL implementation being used
use rp_pico as bsp;
use {
    bsp::{
        entry,
        hal::{
            self,
            clocks::{init_clocks_and_plls, Clock, ClocksManager},
            multicore::{Multicore, Stack},
            pac,
            sio::Sio,
            watchdog::Watchdog,
        },
    },
    defmt::*,
    defmt_rtt as _,
    embedded_hal::digital::v2::OutputPin,
    panic_probe as _,
    usb_device::{class_prelude::*, prelude::*},
    usbd_midi::{
        data::{
            usb::constants::{USB_AUDIO_CLASS, USB_MIDISTREAMING_SUBCLASS},
            usb_midi::midi_packet_reader::MidiPacketBufferReader,
        },
        midi_device::MidiClass,
    },
};

static mut CORE1_STACK: Stack<4096> = Stack::new();

/// External high-speed crystal on the pico board is 12Mhz
const EXTERNAL_XTAL_FREQ_HZ: u32 = 12_000_000u32;

/// Vendor ID, from https://pid.codes
const USB_VID: u16 = 0x1209;
/// Product ID for Project Cipher
// Note(Harry): Request unique PID from https://pid.codes
const USB_PID: u16 = 0x0001;

const USB_MANUFACTURER: &str = "V3X Labs";
const USB_PRODUCT: &str = "Project Cipher";
// Note(HarryET): Devices should load their own serial number from ROM
const USB_SERIAL_NUMBER: &str = "0000-0000-0000-0000";

/// The USB Bus Driver (shared with the interrupt).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

fn core1_main() -> ! {
    let mut pac = unsafe { pac::Peripherals::steal() };
    // let core = unsafe { pac::CorePeripherals::steal() };
    let clocks = ClocksManager::new(pac.CLOCKS);

    // let mut sio = Sio::new(pac.SIO);

    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));
    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_BUS = Some(usb_bus);
    }

    let bus_ref = unsafe { USB_BUS.as_ref().unwrap() };

    let mut usb_midi = MidiClass::new(bus_ref, 1, 1).expect("Unable to create USB MIDI device");

    let mut usb_dev = UsbDeviceBuilder::new(bus_ref, UsbVidPid(USB_VID, USB_PID))
        .manufacturer(USB_MANUFACTURER)
        .product(USB_PRODUCT)
        .serial_number(USB_SERIAL_NUMBER)
        .device_class(USB_AUDIO_CLASS)
        .device_sub_class(USB_MIDISTREAMING_SUBCLASS)
        .build();

    loop {
        if !usb_dev.poll(&mut [&mut usb_midi]) {
            continue;
        }

        let mut buffer = [0; 64];

        if let Ok(size) = usb_midi.read(&mut buffer) {
            let buffer_reader = MidiPacketBufferReader::new(&buffer, size);
            for packet in buffer_reader.into_iter() {
                if let Ok(packet) = packet {
                    match packet.message {
                        // Message::NoteOn(Channel1, Note::C2, ..) => {
                        //     led_pin.set_low().unwrap();
                        // }
                        // Message::NoteOff(Channel1, Note::C2, ..) => {
                        //     led_pin.set_high().unwrap();
                        // }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[entry]
fn core0_main() -> ! {
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let mut sio = Sio::new(pac.SIO);

    let clocks = init_clocks_and_plls(
        EXTERNAL_XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let core1 = &mut cores[1];
    let _ = core1.spawn(unsafe { &mut CORE1_STACK.mem }, core1_main);

    let mut led_pin = pins.led.into_push_pull_output();
    led_pin.set_high().unwrap();

    loop {
        led_pin.set_high().unwrap();
        delay.delay_ms(500);

        led_pin.set_low().unwrap();
        delay.delay_ms(500);
    }
}
