# Urchin

An unikernel for routing.

## Concepts

* Just for virtual machine (especially firecracker)
* No User space. All of codes are running on Kernel(privileged) space.
* No Virtual memory. flattened memory address space.
* Tiny foot print. (~ 1MB)

## Requirements

* Processor supports x86_64, 1GB Hugepage

## How to build

./build.sh

## Todo

* To support multi core
* ECMP
* BGP


