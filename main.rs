#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

extern crate uefi;
extern crate uefi_services;

use uefi::prelude::*;

#[entry]
fn uefi_start(_image_handler: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    loop{};
    Status::SUCCESS;
}
