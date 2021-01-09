use std::{convert::TryInto, error::Error, fmt, fs::File, io, io::prelude::*, sync::{Arc, Barrier, RwLock, atomic::{AtomicI32, Ordering}}, thread::sleep, time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH}};

use hdrhistogram::{Histogram, RecordError};
use nats::{self, Connection, Options};

use libm::{floor, log, pow};
use parse_duration::parse;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    /// Server A (Publish)
    #[structopt(long = "sa", default_value = "nats://localhost:4222")]
    server_a: String,

    /// Server B (Subscribe)
    #[structopt(long = "sb", default_value = "nats://localhost:4222")]
    server_b: String,

    /// Message size in bytes
    #[structopt(long = "sz", default_value = "8")]
    msg_size: u32,

    /// Rate in msgs/sec
    #[structopt(long = "tr", default_value = "1000")]
    target_pub_rate: u32,

    /// Test duration
    #[structopt(long = "tt", default_value = "5s")]
    test_duration: String,

    /// Histogram output file
//    #[structopt(long = "hist")]
//    hist_file: Option<String>,

    /// Enable TLS without verification [default: false]
    #[structopt(long = "secure")]
    secure: bool,

    /// TLS Certificate CA file
    #[structopt(long = "tls_ca")]
    tls_ca: Option<String>,

    /// TLS Private file
    #[structopt(long = "tls_key")]
    tls_key: Option<String>,

    /// TLS Certificate
    #[structopt(long = "tls_cert")]
    tls_cert: Option<String>,

    /// User Credentials file
    #[structopt(long = "creds")]
    user_creds: Option<String>,
}

fn main() -> Result<(), LatencyTestError> {
    let start = SystemTime::now();
    env_logger::init();

    let args = Cli::from_args();

    let num_pubs = match parse(&args.test_duration) {
        Ok(d) => d.as_secs() * args.target_pub_rate as u64,
        Err(err) => {
            return Err(LatencyTestError::new(format!(
                "Error converting test duration: {}",
                err
            )));
        }
    };

    if args.msg_size < 8 {
        eprintln!("Message Payload Size must be at least 8 bytes");
        return Ok(());
    }

    let mut options1;
    let mut options2;
    if let Some(credentials) = args.user_creds {
        options1 = Options::with_credentials(std::path::Path::new(&credentials));
        options2 = Options::with_credentials(std::path::Path::new(&credentials));
    } else {
        options1 = Options::new();
        options2 = Options::new();
    }

    if args.secure {
        options1 = options1.tls_required(true);
        options2 = options2.tls_required(true);
    }
    if let (Some(key), Some(cert)) = (args.tls_key, args.tls_cert) {
        options1 = options1.client_cert(&cert, &key);
        options2 = options2.client_cert(&cert, &key);
    }
    if let Some(ca_cert) = args.tls_ca {
        options1 = options1.add_root_certificate(&ca_cert);
        options2 = options2.add_root_certificate(&ca_cert);
    }

    
    let c1 = match options1.connect(&args.server_a) {
        Ok(c) => c,
        Err(e) => {
            return Err(LatencyTestError::new(format!(
                "Could not connect to ServerA: {}",
                e
            )));
        }
    };

    let c2 = match options2.connect(&args.server_b) {
        Ok(c) => c,
        Err(e) => {
            return Err(LatencyTestError::new(format!(
                "Could not connect to ServerB: {}",
                e
            )));
        }
    };

    // Do some quick RTT calculations
    println!("==============================");
    let now = SystemTime::now();
    c1.flush()?;
    println!("Pub Server RTT : {} µs", SystemTime::elapsed(&now)?.as_micros());

    let now = SystemTime::now();
    c2.flush()?;
    println!("Sub Server RTT : {} µs", SystemTime::elapsed(&now)?.as_micros());

    // Duration tracking
    let mut dur = Vec::<Duration>::new();
    dur.reserve_exact(num_pubs as usize);
    let durations = Arc::new(RwLock::new(dur));
    let durations_subscriber_lock = durations.clone();

    // Wait for all messages to be received.
    let barrier = Arc::new(Barrier::new(1));

    // Random subject (to run multiple tests in parallel)
    let subject = c1.new_inbox();

    // Count the messages
    let mut received: u64 = 0;

    // Async Subscribe (Runs in its own thread)
    let sub_subject = subject.clone();
    let c2_c = c2.clone();
    let barrier_c = barrier.clone();
    std::thread::spawn(move || {
        let barrier_c2 = Arc::clone(&barrier_c);
        let sub = c2_c.subscribe(&sub_subject).unwrap();
        if let Ok(mut durations_write_guard) = durations_subscriber_lock.write() {
            for msg in sub.messages() {
                let receive_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                let send_time = Duration::from_nanos(u64::from_le_bytes(msg.data[..8].try_into().unwrap()));
                //println!("latency calculated: {:?}", receive_time - send_time);
                //let d = receive_time.as_nanos() as u64 - u64::from_le_bytes(msg.data[..8].try_into().unwrap());
                durations_write_guard.push(receive_time - send_time);
                received += 1;
                if received >= num_pubs {
                    barrier_c2.wait();
                    break;
                }
            }
        }
    });

    // Make sure interest is set for subscribe before publish since a different connection
    c2.flush()?;

    // Wait for routes to be established so we get every message
    wait_for_route(&c1,&c2, &args.server_a, &args.server_b)?;

    println!("Message Payload: {}", byte_size(args.msg_size));
    println!("Target Duration: {}", args.test_duration);
    println!("Target Msgs/Sec: {}", args.target_pub_rate);
    println!( "Target Band/Sec: {}", bps((args.target_pub_rate * args.msg_size * 2) as u64));
    println!("==============================");

    // Random payload
    let mut data = vec![0u8; args.msg_size as usize];

    // For publishing throttling
    let mut delay = 1.0 / args.target_pub_rate as f64;
    let pub_start = SystemTime::now();

    // Now publish
    for i in 0..num_pubs {
        let now= SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        data.splice(..8, now.to_le_bytes().iter().cloned());
        c1.publish(&subject, &data)?;
        delay = adjust_and_sleep(delay, now, i + 1, args.target_pub_rate, pub_start);
        //println!("Delay is calculated to: {}", delay);
    }

    let pub_dur = SystemTime::elapsed(&pub_start)?;
    barrier.wait();
    let sub_dur = SystemTime::elapsed(&pub_start)?;

    // If we are writing to files, save the original unsorted data
    // TODO: Write durations to file
//    if let Some(file) = args.hist_file {
//        write_raw_file(file, &durations)?;
//    }

    let mut duration_max: u64 = u64::max_value();
    if let Ok(mut durations) = durations.write() {
        durations.sort();
        duration_max = match durations.last() {
            Some(v) => v.as_nanos() as u64,
            None => u64::max_value(),
        };
    }

    let mut h = Histogram::<u64>::new_with_bounds(1, duration_max, 3).unwrap();
    if let Ok(durations) = durations.read() {
        for d in &*durations {
            h.record(d.as_nanos() as u64)?;
        }
    }

    println!("HDR Percentile");
    println!( "10:       {:?}", Duration::from_nanos(h.value_at_percentile(10.0)));
    println!( "50:       {:?}", Duration::from_nanos(h.value_at_percentile(50.0)));
    println!( "75:       {:?}", Duration::from_nanos(h.value_at_percentile(75.0)));
    println!( "90:       {:?}", Duration::from_nanos(h.value_at_percentile(90.0)));
    println!( "99:       {:?}", Duration::from_nanos(h.value_at_percentile(99.0)));
    println!( "99.9:     {:?}", Duration::from_nanos(h.value_at_percentile(99.9)));
    println!( "99.99:    {:?}", Duration::from_nanos(h.value_at_percentile(99.99)));
    println!( "99.999:   {:?}", Duration::from_nanos(h.value_at_percentile(99.999)));
    println!( "99.9999:  {:?}", Duration::from_nanos(h.value_at_percentile(99.9999)));
    println!( "99.99999: {:?}", Duration::from_nanos(h.value_at_percentile(99.99999)));
    println!( "100:      {:?}", Duration::from_nanos(h.value_at_percentile(100.0)));
    println!("==============================");

    // TODO: write procentiles to file
    // See: https://github.com/tylertreat/hdrhistogram-writer/blob/master/writer.go
    // if let Some(file) = args.hist_file {
    // let pctls := hw.Percentiles{10, 25, 50, 75, 90, 99, 99.9, 99.99, 99.999, 99.9999, 99.99999, 100.0}
    // hw.WriteDistributionFile(h, pctls, 1.0/1000000.0, HistFile+".histogram")
    // }

    if let Ok(durations) = durations.read() {
        println!("Actual Msgs/Sec: {}", rps(num_pubs, pub_dur));
        println!(
            "Actual Band/Sec: {}",
            bps((rps(num_pubs, pub_dur) * args.msg_size * 2) as u64)
        );
        println!("Minimum Latency: {:?}", durations[0]);
        println!("Median Latency : {:?}", get_median(&durations));
        println!("Maximum Latency: {:?}", durations[durations.len() - 1]);
        println!("1st Sent Wall Time : {:?}", pub_start.duration_since(start).unwrap());
        println!("Last Sent Wall Time: {:?}", pub_dur);
        println!("Last Recv Wall Time: {:?}", sub_dur);
    }

    Ok(())
}

/// wait_for_route tests a subscription in the server to ensure subject interest
/// has been propagated between servers.  Otherwise, we may miss early messages
/// when testing with clustered servers and the test will hang.
fn wait_for_route(pnc: &Connection, snc: &Connection, publish_server: &str, subscribe_server: &str) -> Result<(), LatencyTestError>{

    // No need to continue if using one server
    // TODO: Implement Connection.server_id() on nats.rs/src/lib.rs like the nats.go ConnectedServerId
	if publish_server == subscribe_server {
        return Ok(());
	}

	// Setup a test subscription to let us know when a message has been received.
	// Use a new inbox subject as to not skew results
	let routed  = Arc::new(AtomicI32::new(0));
    let subject = pnc.new_inbox();
    // TODO: Implement Connection.subscribe_func(&subject, || {...}) like the nats.go Subscribe(subject, func ...)
    //       This way spawning could be moved to the lib

    let snc_c = snc.clone();
    let subject_c = subject.clone();
    let routed_c = routed.clone();
    std::thread::spawn(move || {
        match snc_c.subscribe(&subject_c ) {
            Ok(s) =>  {
                for _ in s.messages() {
                    routed_c.fetch_add(1, Ordering::SeqCst);
                }
            },
            Err(err) => {
                eprintln!("Couldn't subscribe to test subject {}: {}", subject_c, err);
            }
        };
    });
	snc.flush()?;

	// Periodically send messages until the test subscription receives
	// a message.  Allow for two seconds.
	let start = SystemTime::now();
	while routed.load(Ordering::SeqCst) == 0 {
        let now = SystemTime::now();
		if now.duration_since(start).unwrap() > Duration::from_secs(2) {
			eprintln!("Couldn't receive end-to-end test message.")
		}
        pnc.publish(&subject, "")?;
        sleep(Duration::from_millis(10));
    }

    //sub.Unsubscribe();
    Ok(())
}


fn rps(count: u64, elapsed: Duration) -> u32 {
    return (count as f64 / (elapsed.as_nanos() as f64 / 1000_000_000.0)) as u32;
}

// Just pretty print the byte sizes.
fn byte_size(n: u32) -> String {
    let sizes: [char; 5] = ['B', 'K', 'M', 'G', 'T'];
    let base: f64 = 1024.0;
    if n < 10 {
        return format!("{}{}", n, sizes[0]);
    }
    let e = floor(logn(n as f64, base));
    let suffix = sizes[e as usize];
    let val = floor(n as f64 / pow(base, e) * 10.0 + 0.5) / 10.0;
    if val < 10.0 {
        return format!("{:.1}{}", val, suffix);
    }
    return format!("{:.0}{}", val, suffix);
}

fn bps(n: u64) -> String {
    let sizes: [&str; 5] = ["Bps", "Kbps", "Mbps", "Gbps", "Tbps"];
    let base: f64 = 1024.0;
    let nn = n * 8;
    if nn < 10 {
        return format!("{}{}", nn, sizes[0]);
    }
    let e = floor(logn(nn as f64, base));
    let suffix = sizes[e as usize];
    let val = floor(nn as f64 / pow(base, e) * 10.0 + 0.5) / 10.0;
    if val < 10.0 {
        return format!("{:.1}{}", val, suffix);
    }
    return format!("{:.0}{}", val, suffix);
}

fn adjust_and_sleep( mut delay: f64, now_ns: u64, count: u64, target_pub_rate: u32, pub_start: SystemTime) -> f64 {
    let dur = Duration::from_nanos(now_ns) - pub_start.duration_since(UNIX_EPOCH).unwrap();
    let current_rps = rps(count, dur);
    let adj = delay / 20.0; // 5%
    if current_rps < target_pub_rate {
        delay -= adj;
        if delay < 0.0 {
            delay = 0.0;
        }
    } else if current_rps > target_pub_rate {
        delay += adj;
    }

    if delay > 0.0 {
        sleep(Duration::from_secs_f64(delay));
    }
    return delay;
}

fn logn(n: f64, b: f64) -> f64 {
    return log(n) / log(b);
}

fn get_median(values: &Vec<Duration>) -> Duration {
    let l = values.len();
    if l == 0 {
        eprintln!("empty set");
    }
    if l % 2 == 0 {
        return (values[l / 2 - 1] + values[l / 2]) / 2;
    }
    return values[l / 2];
}

/// write_raw_file creates a file with a list of recorded latency measurements, one per line.
#[allow(dead_code)]
fn write_raw_file( file_path: String, values: &Arc<RwLock<Vec<Duration>>>,) -> Result<(), std::io::Error> {
    let mut file = File::create(file_path)?;
    if let Ok(vals) = values.read() {
        for v in &*vals {
            file.write_fmt(format_args!("{}", v.as_nanos()))?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct LatencyTestError {
    details: String,
}

impl LatencyTestError {
    fn new(msg: String) -> LatencyTestError {
        LatencyTestError { details: msg }
    }
}

impl fmt::Display for LatencyTestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for LatencyTestError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl From<SystemTimeError> for LatencyTestError {
    fn from(error: SystemTimeError) -> Self {
        LatencyTestError {
            details: error.to_string(),
        }
    }
}

impl From<io::Error> for LatencyTestError {
    fn from(error: io::Error) -> Self {
        LatencyTestError {
            details: error.to_string(),
        }
    }
}

impl From<RecordError> for LatencyTestError {
    fn from(error: RecordError) -> Self {
        LatencyTestError {
            details: error.to_string(),
        }
    }
}

#[test]
fn bps_test_100000msgps_8k () {
    let res = bps(100000 * 8192 * 2);
    assert_eq!(res, "12Gbps");
}

#[test]
fn bps_test_1000msgps_1k () {
    let res = bps(1000 * 1024 * 2);
    assert_eq!(res, "16Mbps");
}
