#!/bin/sh

#compile components
cargo build --release
make

#link
ld -v -x -nostdlib -m elf_x86_64 -entry=entry64 -o kernel.elf build/startup.o  target/release/libkernel.a
