# touch

Simple touch input reader for Linux evdev devices.

By default it reads `/dev/input/event0` and prints touch-related events in an
`evtest`-style format:

```sh
sudo cargo run
```

To read another input device, pass the device path:

```sh
sudo cargo run -- /dev/input/event1
```
