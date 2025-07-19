# HIDEx: A Simple HID Gadget Proxy in Rust

This project is a simple demonstration of a HID gadget proxy written in Rust.

**HIDEx** is a userspace application that captures mouse events from a Linux input device (`/dev/input/eventX`) and sends them as USB HID reports to a host computer through the USB Gadget interface (`/dev/hidgX`).

This allows a device like a Raspberry Pi to act as a USB mouse, forwarding input events with low latency.

## Performance

Performance of this Rust implementation was tested and compared with a version written in Go. Latency measurements were taken using Linux High-Resolution Timers (HRT).

Below are the results of latency tests conducted on a Raspberry Pi 4B. Latency is defined as the time required for a mouse event on the device to reach the host computer:

| Metric             | Rust (HIDEx)         | Go (Previous)     |
|--------------------|----------------------|-------------------|
| Average latency    | 956 µs               | 2122 µs           |
| Minimum latency    | 321 µs               | 752 µs            |
| Maximum latency    | 1459 µs              | 3116 µs           |
| Median latency     | 937 µs               | 2209 µs           |
| Standard deviation | 352 µs               | 672 µs            |

HIDEx achieves sub-millisecond average latency with significantly reduced jitter.
