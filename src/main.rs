//have a look at this:: https://github.com/katyo/gpiod-rs/tree/master/tokio

use async_nats::{Client, connection::State};
use tokio::sync::broadcast::{self, Receiver};
use chrono::{DateTime, Timelike, prelude::*};
use std::process;

mod ext;
use crate::ext::nrj_io::{do_main, NrjEvent};

fn start_hourly_thread(client: Client, mut rx: Receiver<NrjEvent>){
     //Spawning a 1h cron job for collecting hourly kWh consumption statistics
    tokio::spawn(async move {
        let mut prev_time = Local::now().with_timezone(Local::now().offset());
        let mut cnt = 0;
        loop {
            tokio::select! {
                result = rx.recv() => {
                    let mut msg = result.expect("should have gotten a msg but something went wrong...");
                    let timestamp = DateTime::parse_from_rfc3339(msg.get_timestamp()).expect("Couldnt parse the timestamp");

                    if (timestamp.time().minute() < prev_time.time().minute()) && (prev_time.time().minute() != 0) {
                        msg.duration = 3600.0;
                        msg.consumption = (f64::from(cnt)/f64::from(2000)).into(); 
                        send(msg,"energy.day", client.clone());

                        cnt = 0;
                    }
                    cnt = cnt + 1;
                    prev_time = timestamp;
                }
            }
        }
    });
}

fn start_daily_thread(client: Client, mut rx: Receiver<NrjEvent>){
    //Spawning a 24h cron job for collecting hourly kWh consumption statistics
    tokio::spawn(async move {
        let mut prev_time = Local::now().with_timezone(Local::now().offset());
        let mut cnt = 0;
        loop {
            tokio::select! {
                result = rx.recv() => {
                    let mut msg = result.expect("should have gotten a msg but something went wrong...");
                    let timestamp = DateTime::parse_from_rfc3339(msg.get_timestamp()).expect("Couldnt parse the timestamp");

                    if timestamp.day() != prev_time.day() {
                        msg.duration = 86400.0;
                        msg.consumption = (f64::from(cnt)/f64::from(2000)).into(); 
                        send(msg,"energy.day", client.clone());

                        cnt = 0;
                    }
                    cnt = cnt + 1;
                    prev_time = timestamp;
                }
            }
        }
    });
}

fn start_momentary_thread(client: Client, mut rx: Receiver<NrjEvent>){
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    let msg = result.expect("should have gotten a msg but something went wrong...");
                    send(msg,"energy.momentary", client.clone());
                }
            }
        }
    });
}

fn send(msg: NrjEvent, topic: &str, client: Client) {
    if client.connection_state() == State::Connected {
        let topic = String::from(topic);
        tokio::spawn(async move {
            let res = client.publish((*topic).to_string(), msg.to_string().into()).await;
            match res {
                Err(err) =>eprintln!("{}:: {}",topic, err),
                Ok(value) => value,
            }
        });
    } else {
        eprintln!("{}:: NATS client state:: {:?}",topic, client.connection_state());
    }
}

#[tokio::main]
async fn main() {//-> std::io::Result<()> {
    let client = async_nats::connect("192.168.1.130").await.unwrap_or_else(|err|{
        eprintln!("Couldn't connect to NATS server, exiting...{err}");
        process::exit(1);
    }); 
    println!("NATS connection state: {}", client.connection_state());

    let (tx, mut _rx) = broadcast::channel::<NrjEvent>(10);
    start_hourly_thread(client.clone(), tx.subscribe());
    start_daily_thread(client.clone(), tx.subscribe());
    start_momentary_thread(client.clone(), tx.subscribe());

    do_main("/dev/gpiochip0", 14, tx).await.expect("we exited the event loop")
}


