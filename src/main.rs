use gpio_cdev::{Chip, EventRequestFlags, LineRequestFlags, EventType, LineEvent};


fn do_main(ch :&str, port :u32) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(ch)?;
    let line = chip.get_line(port)?;
    let mut old:Option<LineEvent> = None;

    for event in line.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "gpioevents",
    )? {
        println!("{:?}", &event);
        let evt = event?;
        match evt.event_type() {
            EventType::RisingEdge => {
                if (old )
                println!("{:?}", evt);
            }
            EventType::FallingEdge => {
                println!("{:?}", evt);
            }
        }
    }

    Ok(())
}

fn main() {
    let res = do_main("/dev/gpiochip0", 16);

    println!("{:?}",res);
}