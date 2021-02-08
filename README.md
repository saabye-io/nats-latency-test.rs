# NATS Latency and Throughput Test Framework

This is a port of https://github.com/nats-io/latency-tests written in RUST. All credits go to them!

This porting was done to verify was I was seeing using NATS, namely that the Nats-rust client was not at fast as the Nats-go client. 


## Update - 2021-02-08

After nats.rs was updated to version 0.9.3 it can compete with the GO lib as the ~40ms timeouts is gone.

See: https://github.com/nats-io/nats.rs/issues/85 and https://github.com/nats-io/nats.rs/pull/141

```
$ ./latency-rs --tr 100 --tt 1s --sz 1024
==============================
Pub Server RTT : 160 µs
Sub Server RTT : 112 µs
Message Payload: 1.0K
Target Duration: 1s
Target Msgs/Sec: 100
Target Band/Sec: 1.6Mbps
==============================
HDR Percentile
10:       120.127µs
50:       136.319µs
75:       151.167µs
90:       173.183µs
99:       264.447µs
99.9:     334.591µs
99.99:    334.591µs
99.999:   334.591µs
99.9999:  334.591µs
99.99999: 334.591µs
100:      334.591µs
==============================
Actual Msgs/Sec: 98
Actual Band/Sec: 1.5Mbps
Minimum Latency: 116.439µs
Median Latency : 136.571µs
Maximum Latency: 334.478µs
1st Sent Wall Time : 893.560965ms
Last Sent Wall Time: 1.011699727s
Last Recv Wall Time: 1.011700839s

```

And even with some bigger numbers it is looking good:

```
$ ./latency-rs --tr 10000 --tt 1s --sz 16184
==============================
Pub Server RTT : 127 µs
Sub Server RTT : 136 µs
Message Payload: 16K
Target Duration: 1s
Target Msgs/Sec: 10000
Target Band/Sec: 2.4Gbps
==============================
HDR Percentile
10:       108.095µs
50:       198.399µs
75:       254.847µs
90:       307.711µs
99:       534.015µs
99.9:     756.735µs
99.99:    858.111µs
99.999:   942.079µs
99.9999:  942.079µs
99.99999: 942.079µs
100:      942.079µs
==============================
Actual Msgs/Sec: 9981
Actual Band/Sec: 2.4Gbps
Minimum Latency: 81.012µs
Median Latency : 198.377µs
Maximum Latency: 941.766µs
1st Sent Wall Time : 1.24520117s
Last Sent Wall Time: 1.001848448s
Last Recv Wall Time: 1.001849199s
```


### Install
```
$ git clone github.com/saabye-io/nats-latency-test.rs && cd nats-latency-test.rs 
```

### Running a local NATS server.

You do not need a local server to run the test framework. However if you want to do so, the recommended way to install the NATS server is to [download](http://nats.io/download/) one of the pre-built release binaries which are available for OSX, Linux (x86-64/ARM), Windows, and Docker. Instructions for using these binaries are on the [GitHub releases page][github-release].

[github-release]: https://github.com/nats-io/gnatsd/releases/

#### Fast run

A Docker Compose file is supplied, just start it with:
```bash
$ docker-compose up -d
```

### Running a test.

_NOTE: TSL with flag --secure is not tested_

```bash
$ cargo build --release && cp target/release/latency-rs .
$ ./latency-rs -h

nats-latency-test-rs 0.1.0

USAGE:
    latency-rs [FLAGS] [OPTIONS]

FLAGS:
    -h, --help       Prints help information
        --secure     Enable TLS without verification [default: false]
    -V, --version    Prints version information

OPTIONS:
        --sz <msg-size>           Message size in bytes [default: 8]
        --sa <server-a>           Server A (Publish) [default: nats://localhost:4222]
        --sb <server-b>           Server B (Subscribe) [default: nats://localhost:4222]
        --tr <target-pub-rate>    Rate in msgs/sec [default: 1000]
        --tt <test-duration>      Test duration [default: 5s]
        --tls_ca <tls-ca>         TLS Certificate CA file
        --tls_cert <tls-cert>     TLS Certificate
        --tls_key <tls-key>       TLS Private file
        --creds <user-creds>      User Credentials file

```

The test framework will run a test to publish and subscribe to messages. Publish operations will happen on one connection to ServerA, and Subscriptions will be on another connection to ServerB. ServerA and ServerB can be the same server.

You are able to specify various options such as message size [-sz], transmit rate [-tr], test time duration [-tt], and output file for plotting with http://hdrhistogram.github.io/HdrHistogram/plotFiles.html.

### Examples

**Basic use with the supplied docker-compose file:**

```bash
$ ./latency-rs --tr 1000 --tt 5s
```
This example will connect both connections to the local (docker) server, attempting to send at 1000 msgs/sec with each message payload being 8 bytes long (default value). This test duration will be ~5 seconds.

**Or with specific servers:**
```bash
$ ./latency-rs --sa tls://demo.nats.io:4443 --sb tls://demo.nats.io:4443 --tr 1000 --tt 5s --sz 512
```

This example will connect both connections to a secure demo server, attempting to send at 1000 msgs/sec with each message payload being 512 bytes long. This test duration will be ~5 seconds.

### Output

```text
==============================
Pub Server RTT : 1.65ms
Sub Server RTT : 2.817ms
Message Payload: 512B
Target Duration: 5s
Target Msgs/Sec: 1000
Target Band/Sec: 1000K
==============================
HDR Percentiles:
10:       1.998ms
50:       2.058ms
75:       2.095ms
90:       2.132ms
99:       2.271ms
99.99:    3.106ms
99.999:   3.126ms
99.9999:  3.126ms
99.99999: 3.126ms
100:      3.126ms
==============================
Actual Msgs/Sec: 998
Actual Band/Sec: 998K
Minimum Latency: 1.919ms
Median Latency : 2.058ms
Maximum Latency: 3.126ms
1st Sent Wall Time : 153.489ms
Last Sent Wall Time: 5.005243s
Last Recv Wall Time: 5.006857s
```

This is output from the previous example run. The test framework will establish a rough estimate of the RTT to each server via a call to ``nats.Flush()``. The message payload size, test duration and target msgs/sec and subsequent bandwidth will be noted. After the test completes the histogram percentiles for 10th, 50th, 75th, 90th, 99th,  99.99th, 99.999th, 99.9999th, 99.99999th, and 100th percentiles are printed.  After this, we print the actual results of achieved msgs/sec, bandwidth/sec, the minimum, median, and maximum latencies, and wall times recorded in the test run.  Note that the number of measurements (total messages) may cause overlap in the highest percential latency measurements, as demonstrated in the output above with 5000 measurements.

## RUST NATS client (using nats v0.8.6) vs GO Nats client

I ran these tests to get a feeling of the performance and it looks really good... For the GO NATS client. 


Rust and go version is: 
```
$ rustc --version
rustc 1.48.0 (7eac88abb 2020-11-16)

$ go version
go version go1.15.6 linux/amd64
```

### Small payloads short time

GO wins as RUST is around 40% slower...

```
$ ./latency-rs --tr 10 --tt 1s --sz 8
==============================
Pub Server RTT : 201 µs
Sub Server RTT : 148 µs
Message Payload: 8B
Target Duration: 1s
Target Msgs/Sec: 10
Target Band/Sec: 1.3Kbps
==============================
HDR Percentile
10:       201.727µs
50:       841.215µs
75:       891.391µs
90:       927.231µs
99:       930.815µs
99.9:     930.815µs
99.99:    930.815µs
99.999:   930.815µs
99.9999:  930.815µs
99.99999: 930.815µs
100:      930.815µs
==============================
Actual Msgs/Sec: 8
Actual Band/Sec: 1.0Kbps
Minimum Latency: 201.655µs
Median Latency : 843.97µs
Maximum Latency: 930.361µs
1st Sent Wall Time : 1.134744281s
Last Sent Wall Time: 1.159599408s
Last Recv Wall Time: 1.159602661s
```
🠉 RUST   🠋 GO

```
$ ./latency -tr 10 -tt 1s -sz 8
==============================
Pub Server RTT : 102µs
Sub Server RTT : 58µs
Message Payload: 8B
Target Duration: 1s
Target Msgs/Sec: 10
Target Band/Sec: 1.3Kbps
==============================
HDR Percentiles:
10:       95µs
50:       375µs
75:       560µs
90:       581µs
99:       675µs
99.9:     675µs
99.99:    675µs
99.999:   675µs
99.9999:  675µs
99.99999: 675µs
100:      675µs
==============================
Actual Msgs/Sec: 8
Actual Band/Sec: 1.0Kbps
Minimum Latency: 95µs
Median Latency : 419µs
Maximum Latency: 675µs
1st Sent Wall Time : 1.662ms
Last Sent Wall Time: 1.159426s
Last Recv Wall Time: 1.159427s
```

### Small payloads over 5s - Go wins

Note: Look at the 90% percentile is is almost the same as the 1s test.


```
$ ./latency-rs --tr 10 --tt 5s --sz 8
==============================
Pub Server RTT : 215 µs
Sub Server RTT : 135 µs
Message Payload: 8B
Target Duration: 5s
Target Msgs/Sec: 10
Target Band/Sec: 1.3Kbps
==============================
HDR Percentile
10:       711.167µs
50:       884.223µs
75:       935.935µs
90:       963.583µs
99:       1.144831ms
99.9:     1.144831ms
99.99:    1.144831ms
99.999:   1.144831ms
99.9999:  1.144831ms
99.99999: 1.144831ms
100:      1.144831ms
==============================
Actual Msgs/Sec: 10
Actual Band/Sec: 1.3Kbps
Minimum Latency: 219.094µs
Median Latency : 886.357µs
Maximum Latency: 1.144723ms
1st Sent Wall Time : 1.700579233s
Last Sent Wall Time: 4.936224531s
Last Recv Wall Time: 4.936226273s
```
🠉 RUST   🠋 GO
```
$ ./latency -tr 10 -tt 5s -sz 8
==============================
Pub Server RTT : 80µs
Sub Server RTT : 68µs
Message Payload: 8B
Target Duration: 5s
Target Msgs/Sec: 10
Target Band/Sec: 1.3Kbps
==============================
HDR Percentiles:
10:       382µs
50:       611µs
75:       655µs
90:       663µs
99:       722µs
99.9:     722µs
99.99:    722µs
99.999:   722µs
99.9999:  722µs
99.99999: 722µs
100:      722µs
==============================
Actual Msgs/Sec: 10
Actual Band/Sec: 1.3Kbps
Minimum Latency: 102µs
Median Latency : 612µs
Maximum Latency: 722µs
1st Sent Wall Time : 1.551ms
Last Sent Wall Time: 4.938808s
Last Recv Wall Time: 4.938812s
```


### Small payload 1000 msg/sec 

Remember to look at the 90/99 percentile. 

The timings of ~41ms could be messages sent while a garbage collection is ongoing ?

After multiple tries I gave up trying to get the GO client to return time around 40ms. 

```
$ ./latency-rs --tr 1000 --tt 5s --sz 8
==============================
Pub Server RTT : 199 µs
Sub Server RTT : 193 µs
Message Payload: 8B
Target Duration: 5s
Target Msgs/Sec: 1000
Target Band/Sec: 125Kbps
==============================
HDR Percentile
10:       534.015µs
50:       826.879µs
75:       1.156095ms
90:       1.401855ms
99:       1.666047ms
99.9:     39.223295ms
99.99:    40.959999ms
99.999:   40.959999ms
99.9999:  40.959999ms
99.99999: 40.959999ms
100:      40.959999ms
==============================
Actual Msgs/Sec: 1001
Actual Band/Sec: 125Kbps
Minimum Latency: 151.412µs
Median Latency : 826.926µs
Maximum Latency: 40.92884ms
1st Sent Wall Time : 1.89079904s
Last Sent Wall Time: 4.990158477s
Last Recv Wall Time: 4.990160306s
```
🠉 RUST   🠋 GO
```
$ ./latency -tr 1000 -tt 5s -sz 8
==============================
Pub Server RTT : 112µs
Sub Server RTT : 71µs
Message Payload: 8B
Target Duration: 5s
Target Msgs/Sec: 1000
Target Band/Sec: 125Kbps
==============================
HDR Percentiles:
10:       307µs
50:       372µs
75:       405µs
90:       436µs
99:       620µs
99.9:     666µs
99.99:    701µs
99.999:   1.475ms
99.9999:  1.475ms
99.99999: 1.475ms
100:      1.475ms
==============================
Actual Msgs/Sec: 998
Actual Band/Sec: 125Kbps
Minimum Latency: 54µs
Median Latency : 372µs
Maximum Latency: 1.475ms
1st Sent Wall Time : 1.398ms
Last Sent Wall Time: 5.005895s
Last Recv Wall Time: 5.005896s
```

### Large (1KB) payload at 1000 msg/sec

Now things begins to slow down...  for rust. Go is unaffected.

```
$ ./latency-rs --tr 1000 --tt 5s --sz 1024
==============================
Pub Server RTT : 240 µs
Sub Server RTT : 161 µs
Message Payload: 1.0K
Target Duration: 5s
Target Msgs/Sec: 1000
Target Band/Sec: 16Mbps
==============================
HDR Percentile
10:       1.258495ms
50:       12.533759ms
75:       22.478847ms
90:       31.064063ms
99:       39.518207ms
99.9:     41.123839ms
99.99:    41.615359ms
99.999:   41.615359ms
99.9999:  41.615359ms
99.99999: 41.615359ms
100:      41.615359ms
==============================
Actual Msgs/Sec: 998
Actual Band/Sec: 16Mbps
Minimum Latency: 104.951µs
Median Latency : 12.530727ms
Maximum Latency: 41.594659ms
1st Sent Wall Time : 779.013513ms
Last Sent Wall Time: 5.009423333s
Last Recv Wall Time: 5.009425743s
```
🠉 RUST   🠋 GO
```
$ ./latency -tr 1000 -tt 5s -sz 1024
==============================
Pub Server RTT : 77µs
Sub Server RTT : 63µs
Message Payload: 1.0K
Target Duration: 5s
Target Msgs/Sec: 1000
Target Band/Sec: 16Mbps
==============================
HDR Percentiles:
10:       241µs
50:       322µs
75:       366µs
90:       412µs
99:       566µs
99.9:     670µs
99.99:    718µs
99.999:   725µs
99.9999:  725µs
99.99999: 725µs
100:      725µs
==============================
Actual Msgs/Sec: 1001
Actual Band/Sec: 16Mbps
Minimum Latency: 61µs
Median Latency : 322µs
Maximum Latency: 725µs
1st Sent Wall Time : 1.479ms
Last Sent Wall Time: 4.990896s
Last Recv Wall Time: 4.990897s
```

### Larger (4KB) payload at 100.000 msg /sec

The 90 percentile is stille pretty good. 

However, ALL the messages for GO is delivered within 3.7ms which is not the case for the RUST client. 

```
$ ./latency-rs --tr 100000 --tt 5s --sz 4096
==============================
Pub Server RTT : 223 µs
Sub Server RTT : 171 µs
Message Payload: 4.0K
Target Duration: 5s
Target Msgs/Sec: 100000
Target Band/Sec: 6.1Gbps
==============================
HDR Percentile
10:       274.687µs
50:       463.615µs
75:       652.287µs
90:       964.095µs
99:       6.578175ms
99.9:     15.286271ms
99.99:    17.891327ms
99.999:   41.713663ms
99.9999:  41.746431ms
99.99999: 41.746431ms
100:      41.746431ms
==============================
Actual Msgs/Sec: 100003
Actual Band/Sec: 2.1Gbps
Minimum Latency: 99.4µs
Median Latency : 463.371µs
Maximum Latency: 41.738451ms
1st Sent Wall Time : 1.074015006s
Last Sent Wall Time: 4.999818752s
Last Recv Wall Time: 4.999819444s
```
🠉 RUST   🠋 GO
```
$ ./latency -tr 100000 -tt 5s -sz 4096
==============================
Pub Server RTT : 77µs
Sub Server RTT : 51µs
Message Payload: 4.0K
Target Duration: 5s
Target Msgs/Sec: 100000
Target Band/Sec: 6.1Gbps
==============================
HDR Percentiles:
10:       41µs
50:       80µs
75:       148µs
90:       298µs
99:       876µs
99.9:     3.211ms
99.99:    3.659ms
99.999:   3.694ms
99.9999:  3.697ms
99.99999: 3.697ms
100:      3.697ms
==============================
Actual Msgs/Sec: 100007
Actual Band/Sec: 6.1Gbps
Minimum Latency: 14µs
Median Latency : 80µs
Maximum Latency: 3.697ms
1st Sent Wall Time : 1.52ms
Last Sent Wall Time: 4.999605s
Last Recv Wall Time: 4.999638s
```
