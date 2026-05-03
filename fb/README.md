This is a simple test outputting to /dev/fb0 on the CC2

```
cross build --release --target armv7-unknown-linux-musleabihf
sshpass -p MTY4ODE2 scp ./target/armv7-unknown-linux-musleabihf/release/fb root@<CC2 ADDRESS>:/root/
```
