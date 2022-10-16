//have a look at this:: https://github.com/katyo/gpiod-rs/tree/master/tokio

use async_nats::Client;
use gpio_cdev::{Chip, EventRequestFlags, LineRequestFlags, EventType, LineEvent};
use tokio::{time::{self, sleep}, sync::watch::{self, Receiver}};
use std::{fmt::Write, time::{Duration, SystemTime}, thread};
use chrono::{DateTime, Utc, Timelike};

async fn do_main(ch :&str, port :u32, client :&Client) -> std::result::Result<(), Box<dyn std::error::Error>> {
    //-> std::result::Result<(), gpio_cdev::Error> {
    println!("do_main start");
    let mut chip = Chip::new(ch)?;
    let input = chip.get_line(port)?;
//    let output = chip.get_line(14)?;
//    let _output_handle = output.request(LineRequestFlags::OUTPUT, 0, "mirror-gpio")?;
    println!("do_main configured and connected");

    let mut old:Option<LineEvent> = None;

        /* test code for checking the period
        thread::spawn(move|| {
            let period = 2000;
            loop {
                output_handle.set_value(1).unwrap();
                thread::sleep(Duration::from_millis(period/2));
                output_handle.set_value(0).unwrap();
                thread::sleep(Duration::from_millis(period/2));
            }
         });
        */
        
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
                        println!("period(s):: {:.3}, Current Consumption::{:.2} kwh", &period, period_to_kwh(&period));
                        //println!("period(s):: {:?}\nnew_ts::{:?}\nold_ts::{:?}", period as f64/1000000000 as f64,  evt.timestamp(), old.unwrap().timestamp());
                        //let result  = sprintf!("period(s):: {:?}\nnew_ts::{:?}\nold_ts::{:?}", period as f64/1000000000 as f64,  evt.timestamp(), old.unwrap().timestamp()).unwrap();
                        //let result  = sprintf!("period(s):: {:.3}, Current Consumption::{:.2} kwh", period, calckwh(&period)).unwrap();
			            let mut result = String::new();
                        write!(result, "period(s):: {:.3}, Current Consumption::{:.2} kwh", period, period_to_kwh(&period)).unwrap();
                        client.publish("nrjmeter".to_string(), result.into()).await?;
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

fn start_hour_thread(){
     //Spawning a 1h cron job for collecting hourly kWh consumption statistics
    tokio::spawn(async move {
        let mut datetime: DateTime<Utc> = SystemTime::now().into();
        println!("timeloop started:: {}", datetime.format("%Y%m%d %T"));
        
        // sync to systemtime, aka will run every hour regardless of when the program was started
        sleep(Duration::from_secs((3600-datetime.minute()*60-datetime.second())as u64)).await;
        let mut interval = time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            datetime = SystemTime::now().into();
            println!("timeTick:: {}", datetime.format("%Y%m%d %T"));
            //TODO I should do something more here :)
        }
    });
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    println!("Start main loop");
    let (tx, mut rx) = watch::channel("hello");
    start_hour_thread();

    //    let nc = nats::connect("192.168.1.130")?;
    let client = async_nats::connect("192.168.1.130").await?;

    println!("connection to nats done");
    let res = do_main("/dev/gpiochip0", 14, &client);
    println!("{:?}",res.await);
    thread::sleep(Duration::from_secs(20));
    Ok(())
}

fn period_to_kwh(period :&f64)-> f64 {
    let kwhper_blinks :u32 = 2000;
    let blinksper_hour :f64 = 3600.0/period;
    blinksper_hour/kwhper_blinks as f64
}
