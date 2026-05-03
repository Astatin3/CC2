cross build --release --target armv7-unknown-linux-musleabihf
sshpass -p MTY4ODE2 scp ./target/armv7-unknown-linux-musleabihf/release/serial-test root@192.168.0.86:/root/
