
use chrono::Local;
use gpio_cdev::{Chip, EventRequestFlags, LineRequestFlags, EventType, LineEvent};
use tokio::sync::watch::Sender;
use serde::{Deserialize, Serialize};

    pub async fn do_main(ch :&str, port :u32, tx: Sender<NrjEvent>) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut chip = Chip::new(ch)?;
        let input = chip.get_line(port)?;
        println!("do_main configured and connected");

        let mut prev:Option<LineEvent> = None;
            
        for event in input.events(
            LineRequestFlags::INPUT,
            EventRequestFlags::BOTH_EDGES,
            "gpioevents")? {
                let evt = event?;
                match evt.event_type() {
                    EventType::RisingEdge => {
                        if prev.is_some() {
                            let period = calculate_period(prev.as_ref().unwrap(), &evt);

                            if period > 0.01 {
                                tx.send(NrjEvent::new(period))?;
                            }                    
                        }
                        prev = Some(evt);
                    }
                    EventType::FallingEdge => {
                    }
                }
            }
        Ok(())
    }
    
    fn calculate_period(prev: &LineEvent, curr: &LineEvent) -> f64 {
        (curr.timestamp() - prev.timestamp()) as f64 / 1000000000_f64
    }

    fn period_to_kwh(period :f64)-> f64 {
        let kwhper_blinks :u32 = 2000;
        let blinksper_hour :f64 = 3600.0/period;
        blinksper_hour/kwhper_blinks as f64
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NrjEvent {
        pub timestamp: String, // Timestamp of the event
        pub duration: f64, // duration since last event (in seconds)
        pub consumption: f64, // Current consumption (in kilowatt-hours)
    }
    impl NrjEvent {
        pub fn new(period :f64) -> NrjEvent{
            let mut duration = 0.0;
            let mut consumption = 0.0;
            if period > 0.0 {
                duration =  (period*1000.0).round() / 1000.0;
                consumption =  (period_to_kwh(period)*100.0).round() / 100.0;
            }

            NrjEvent {
                timestamp: Local::now().to_rfc3339(), // can be decoded using DateTime::parse_from_rfc3339(<timestamp>)
                duration,
                consumption,
            }
        }
        pub fn to_json_string(&self) -> String{
            serde_json::to_string(&self).unwrap()
        }
        pub fn get_timestamp(&self) -> &str {
            self.timestamp.as_str()
        }
    }