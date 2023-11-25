//use rpi_gpio::process;
mod rpi_gpio;

use async_nats::connect;
use rpi_gpio::NrjEvent;
use tokio::sync::watch::{self, Receiver};
use gpiocdev::{Request,line::EdgeDetection, tokio::AsyncRequest};
use std::{time::Duration, env, process};
use anyhow::{Context, Ok};
use tracing::{info, error, debug};
// use tracing_subscriber;

#[tokio::main]
async fn main() {
    // use RUST_LOG=info|debug|trace|error|warn|off to set the log level...
    //RUN_LOG=info ./nrjmeter
    tracing_subscriber::fmt::init();

    // setup the one to many channel
    let (tx, mut _rx) = watch::channel::<NrjEvent>(NrjEvent::default());

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://192.168.2.5:4444".into());
    info!("Connecting to NATS server at {}", nats_url);

    // run the event handler in a separate task to avoid blocking the main thread 
    let rx: Receiver<NrjEvent> = tx.subscribe();
    tokio::spawn(async move {
        nrj_event_handler(rx).await;
    });

    let rx: Receiver<NrjEvent> = tx.subscribe();
    tokio::spawn(async move {
        event_handler_nats(nats_url, rx).await;
    });

    // setup GPIO line request for rising edge detection with 10ms debounce period
    let chip = "/dev/gpiochip0";
    let line = 14;
    let debounce_period = Duration::from_millis(10);

    let req = get_async_request(chip, line, debounce_period).unwrap_or_else(|err| {
        error!("Error requesting GPIO line: {}", err);
        process::exit(1);
    });

    rpi_gpio::process_line_events(req, tx).await.expect("Error processing GPIO");
}


async fn event_handler_nats(nats_url: String, mut rx: Receiver<NrjEvent>){
    let client = connect(nats_url).await.unwrap_or_else(|err|{
        error!("Couldn't connect to NATS server, exiting... {}", err);
        process::exit(1);
    }); 
    
    info!("NATS connection state: {}", client.connection_state());

    loop {
        tokio::select! {
            _result = rx.changed() => {
                let borrowed_rx = rx.borrow().clone();
                
                if let Some(events) = borrowed_rx.get_json_events() {
                    for (subject, payload) in events {
                        if let Err(err) = client.publish(subject, payload.into()).await {
                            error!("Error publishing to NATS server: {}", err);
                        }
                    }
                }
            }
        }
    }
}

fn get_async_request(chip: &str, line: u32, debounce_period: Duration) -> Result<AsyncRequest, anyhow::Error> {//AsyncRequest {
    let request = AsyncRequest::new(Request::builder()
        .on_chip(chip)
        .with_line(line)
        .with_consumer("nrjmeter")
        .with_edge_detection(EdgeDetection::RisingEdge)
        .with_debounce_period(debounce_period)
        .request()
        .context("Error requesting GPIO line")?);
    Ok(request)
}

async fn nrj_event_handler(mut rx : Receiver<NrjEvent>){
    loop {
        tokio::select! {
            _result = rx.changed() => {
                let event = rx.borrow().clone(); 
                match event.event_type {
                    rpi_gpio::NrjEventState::Instant => debug!("Current: {:.2} kW",
                                                                event.pwr_current),
                    rpi_gpio::NrjEventState::Hourly  => debug!("Current: {:.2} kW, Hourly: {:.2} kWh",
                                                                event.pwr_current, event.pwr_hour),
                    rpi_gpio::NrjEventState::Daily   => debug!("Current: {:.2} kW, Hourly: {:.2} kWh, Daily: {:.2} kWh",
                                                                event.pwr_current, event.pwr_hour, event.pwr_day),
                    rpi_gpio::NrjEventState::Unknown => continue,
                }
            }
        }           
    }
}