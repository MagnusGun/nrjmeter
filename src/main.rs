use gpio_cdev::{Chip, EventRequestFlags, LineRequestFlags, EventType, LineEvent};
use std::thread;
use std::time::Duration;

fn do_main(ch :&str, port :u32) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(ch)?;
    let input = chip.get_line(port)?;
    let output = chip.get_line(23)?;
    let output_handle = output.request(LineRequestFlags::OUTPUT, 0, "mirror-gpio")?;


    let mut old:Option<LineEvent> = None;

        // test code for checking the period
        thread::spawn(move|| {
            let period = 2000;
            loop {
                output_handle.set_value(1).unwrap();
                thread::sleep(Duration::from_millis(period/2));
                output_handle.set_value(0).unwrap();
                thread::sleep(Duration::from_millis(period/2));
            }
         });

    for event in input.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "gpioevents",
    )? {
        //println!("{:?}", &event);
        let evt = event?;
        match evt.event_type() {
            EventType::RisingEdge => {
  //              println!("{:?}", &evt);
                if !old.is_none() {
                    let period :f64 = (evt.timestamp() - old.as_ref().unwrap().timestamp()) as f64 /1000000000 as f64;
                    if period > 0.01 {
                        println!("period(s):: {:.3}, Current Consumption::{:.2} kwh", &period, calckwh(period));
                    }
                }
                old = Some(evt);
            }
            EventType::FallingEdge => {
//                println!("{:?}", evt);
            }
        }
    }

    Ok(())
}

fn calckwh(period :f64)-> f64 {
    let kwhper_blinks :u32 = 2000;
    let blinksper_hour :f64 = (1.0/period)*(60*60) as f64;
    blinksper_hour/kwhper_blinks as f64
}

fn main() {
    let res = do_main("/dev/gpiochip0", 16);

    println!("{:?}",res);
}