#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]
#![feature(alloc)]

extern crate uefi;
extern crate uefi_services;
extern crate rlibc;
extern crate alloc;
extern crate log;

use uefi::prelude::*;
use crate::alloc::vec::Vec;
use uefi::table::boot::MemoryType;
use log::info;

const EFI_PAGE_SIZE: u64 = 0x1000;

#[entry]
fn uefi_start(_image_handler: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utils");
    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset output buffer");
    {
        let rev = system_table.uefi_revision();
        let (major, minor) = (rev.major(), rev.minor());
        info!("UEFI {}.{}", major, minor);
        memory_map(&system_table.boot_services());
    }
    loop{};
    Status::SUCCESS;
}

fn memory_map(bt: &BootServices) {
    let map_size = bt.memory_map_size();
    let mut buffer = Vec::with_capacity(map_size);
    unsafe {
        buffer.set_len(map_size);
    }

    let (_k, desc_iter) = bt
        .memory_map(&mut buffer)
        .expect_success("Failed to retrieve UEFI memory map");

    let descriptors = desc_iter.copied().collect::<Vec<_>>();

    assert!(!descriptors.is_empty(), "Memory map is empty");

    info!("efi: usable memory ranges ({} total)", descriptors.len());
    descriptors
        .iter()
        .for_each(|descriptor| match descriptor.ty {
            MemoryType::CONVENTIONAL => {
                let size = descriptor.page_count * EFI_PAGE_SIZE;
                let end_address = descriptor.phys_start + size;
                info!(
                    "> {:#x} - {:#x} ({} KiB)",
                    descriptor.phys_start, end_address, size
                );
            }
            _ => {}
        })
}