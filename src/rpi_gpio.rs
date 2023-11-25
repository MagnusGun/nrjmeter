

use chrono::{Local, DateTime, Timelike, Datelike};
use gpiocdev::{line::EdgeEvent, tokio::AsyncRequest};
use serde::{Serialize, Deserialize};
use tokio::sync::watch::Sender;
use tracing::{info, error, debug};
use tracing_attributes::instrument;
use std::fmt;

const PULSE_PER_KWH: f64 = 2000_f64;
const SECONDS_PER_HOUR: f64 = 3600_f64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NrjEventState {
    Instant,
    Hourly,
    Daily,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct NrjEvent {
    pub event_type: NrjEventState,
    pub pwr_current: f64,
    pub pwr_hour: f64,
    pub pwr_day: f64,
    pub timestamp: DateTime<Local>, 
    prev_event: Option<u64>,
    hour_cnt: u32,
    day_cnt: u32,
}

impl Default for NrjEvent {
    fn default() -> Self {
        Self {
            event_type: NrjEventState::Unknown,
            pwr_current: 0_f64,
            pwr_hour: 0_f64,
            pwr_day: 0_f64,
            timestamp: Local::now(),
            prev_event: None,           
            hour_cnt: 0,
            day_cnt: 0,
        }
    }
}

impl fmt::Display for NrjEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Customize how you want to format NrjEvent here
        write!(f, "NrjEvent {{ /* fields and formatting here */ }}")
    }
}

impl NrjEvent {    
    fn calc_momentary_power(&mut self, event: &EdgeEvent, pe: u64) -> Result<(), String> {
        
        // calculate the duration of the event in seconds and convert it to f64, precision
        // should not be a problem since the duration is a delta between current and previous event
        #[allow(clippy::cast_precision_loss)]
        let duration = (event.timestamp_ns - pe) as f64 / 1e9_f64;

        if duration > 0_f64 {
            //calculate power consumption in kWh where 2000 is the number of pulses per kWh
            let momentary_power = SECONDS_PER_HOUR / (duration * PULSE_PER_KWH);
            // generate a new error if the calculated power is NaN or infinite
            if momentary_power.is_nan() || momentary_power.is_infinite() {
                Err(String::from("Invalid momentary_power, infinite or NaN"))
            } else {
                self.pwr_current = momentary_power;
                Ok(())
            }
        }else { Err(String::from("Invalid event duration, duration is zero or negative"))}
    }

    pub fn get_json_events(&self) -> Option<Vec<(String, String)>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct JsonEvent {
            pub timestamp: String,
            pub power: f64,
        }

        impl JsonEvent {
            fn new(timestamp: DateTime<Local>, power: f64) -> Self {
                Self {
                    timestamp: timestamp.to_rfc3339(),
                    power,
                }
            }
        }

        match self.event_type {
            NrjEventState::Instant => {
                let instant = serde_json::to_string(&JsonEvent::new(self.timestamp, self.pwr_current)).unwrap();
                Some(vec![(String::from("energy.instant"), instant)])
            },
            NrjEventState::Hourly => {
                let instant = serde_json::to_string(&JsonEvent::new(self.timestamp, self.pwr_current)).unwrap();
                let hour = serde_json::to_string(&JsonEvent::new(self.get_adjusted_timestamp(), self.pwr_hour)).unwrap();
                Some(vec![(String::from("energy.instant"), instant),
                          (String::from("energy.hour"),    hour)])
            },
            NrjEventState::Daily => {
                let adj_timestamp = self.get_adjusted_timestamp();
                let instant = serde_json::to_string(&JsonEvent::new(self.timestamp, self.pwr_current)).unwrap();
                let hour = serde_json::to_string(&JsonEvent::new(adj_timestamp, self.pwr_hour)).unwrap();
                let day = serde_json::to_string(&JsonEvent::new(adj_timestamp, self.pwr_day)).unwrap();
                Some(vec![(String::from("energy.instant"), instant),
                          (String::from("energy.hour"),    hour),
                          (String::from("energy.day"),     day)])
            },
            NrjEventState::Unknown => None,
            
        }
    }

    fn check(&mut self, timestamp: DateTime<Local>) {
        if self.timestamp.day() != timestamp.day() {
            self.event_type = NrjEventState::Daily;
            self.pwr_hour = f64::from(self.hour_cnt) / PULSE_PER_KWH;
            self.pwr_day = f64::from(self.day_cnt) / PULSE_PER_KWH;
            self.hour_cnt = 0;
            self.day_cnt = 0;
        } else if self.timestamp.hour() != timestamp.hour() {
            self.event_type = NrjEventState::Hourly;
            self.pwr_hour = f64::from(self.hour_cnt) / PULSE_PER_KWH;
            self.hour_cnt = 0;
        } else {
            self.event_type = NrjEventState::Instant;
        }

        self.hour_cnt += 1;
        self.day_cnt += 1;
        self.timestamp = timestamp;
    }

    fn update(&mut self, event: &EdgeEvent, timestamp: DateTime<Local>) -> Result<(), String>{
        match self.prev_event {
            Some(pe) => {
                self.calc_momentary_power(event, pe)?;
                self.check(timestamp);
                self.prev_event = Some(event.timestamp_ns);
            }, 
            None => {
                self.prev_event = Some(event.timestamp_ns);
            }
        }
        Ok(())
    }

    // adjust the timestamp to get the hour and day event on the correct day and hour
    fn get_adjusted_timestamp(&self) -> DateTime<Local> {
        let mut timestamp = self.timestamp;
        // we need to adjust the timestamp to get the event on the correct day
        if timestamp.hour() == 0 {
            timestamp = timestamp.with_hour(23).unwrap();
        // we need to adjust the timestamp to get the event in the correct hour
        } else {
            let hour = timestamp.hour() - 1;
            timestamp = timestamp.with_hour(hour).unwrap();
        }
        timestamp = timestamp.with_minute(59).unwrap();
        timestamp = timestamp.with_second(59).unwrap();
        timestamp = timestamp.with_nanosecond(999_999_999).unwrap();
        timestamp
    }

}
//#[instrument]
pub async fn process_line_events(req: AsyncRequest, tx: Sender<NrjEvent>) -> Result<(), Box<dyn std::error::Error>> {
    // store the last event to calculate the consumption
    let mut nrj_event = NrjEvent::default();
    
    loop {
        let event = req.read_edge_event().await?;
        match nrj_event.update(&event, Local::now()) {
            Ok(()) => {
                match tx.send(nrj_event.clone()) {
                    Ok(()) => (),//info!("Event sent: {}", nrj_event);
                    Err(e) => {debug!("Error: {}", e);
                    }
                }
            },
            Err(e) => {error!("Error: {}", e);
        }
        }
    }
}

#[instrument]
pub async fn testmylittlefunction(a: String) {
    info!("testmylittlefunction: {:?}", a);
}