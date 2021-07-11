#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate uefi;
extern crate uefi_services;
extern crate rlibc;
extern crate alloc;
extern crate log;

use byteorder::{LittleEndian, ByteOrder};
use uefi::{
    prelude::*,
    proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType},
    proto::media::fs::SimpleFileSystem,
    table::boot::{MemoryType, AllocateType},
};

const EFI_PAGE_SIZE: usize = 0x1000;

#[entry]
fn uefi_start(_image_handler: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utils");
    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset output buffer");
    let bt = system_table.boot_services();
    // load to tmp space because get file size from elf header
    let fs = bt
        .locate_protocol::<SimpleFileSystem>()
        .unwrap_success();
    let fs = unsafe { &mut *fs.get() };
    let mut root_dir = fs.open_volume().unwrap_success();
    let fh = root_dir
        .open("kernel.elf", FileMode::Read, FileAttribute::READ_ONLY)
        .unwrap_success();
    let file_type = fh.into_type().unwrap_success();
    if let FileType::Regular(mut f) = file_type {
        const TMP_BUF_SIZE: usize = 4000;
        let mut misc_buf = [0u8; TMP_BUF_SIZE];
        let info: &mut FileInfo = f.get_info(&mut misc_buf).unwrap_success();
        let kernel_file_size: u64 = info.file_size();

        let kernel_tmp_p = bt.allocate_pool(MemoryType::LOADER_DATA, kernel_file_size as usize).unwrap_success();
        let mut kernel_tmp_buf = unsafe { core::slice::from_raw_parts_mut(kernel_tmp_p as *mut u8, kernel_file_size as usize) };
        f.read(&mut kernel_tmp_buf).unwrap_success();
        f.close();

        // get kernel size
        use elf_rs::*; // to get kernel file size
        let elf = Elf::from_bytes(&kernel_tmp_buf).unwrap();
        let mut kernel_start = u64::MAX;
        let mut kernel_end = u64::MIN;
        if let Elf::Elf64(ref e) = elf {
            for p in e.program_header_iter() {
                let header = p.ph;
                if matches!(header.ph_type(), ProgramType::LOAD) {
                    let s = header.vaddr();
                    let len = header.memsz();
                    kernel_start = core::cmp::min(kernel_start, s);
                    kernel_end = core::cmp::max(kernel_end, s + len);
                }
            }
        }

        // allocate memory
        let load_len = kernel_end - kernel_start;
        let n_pages = (load_len as usize + 0xfff) / EFI_PAGE_SIZE;
        let kernel_p = bt
            .allocate_pages(
                AllocateType::Address(kernel_start as usize),
                MemoryType::LOADER_DATA,
                n_pages
            )
            .unwrap_success();

        let mut kernel_buf = unsafe { core::slice::from_raw_parts_mut(kernel_p as *mut u8, load_len as usize) };

        // initialize
        for i in 0..load_len as usize {
            kernel_buf[i] = 0;
        }

        // Read kernel to memory
        if let Elf::Elf64(ref e) = elf {
            for p in e.program_header_iter() {
                let header = p.ph;
                if matches!(header.ph_type(), ProgramType::LOAD) {
                    let src = p.segment();
                    let dst_addr = header.vaddr();
                    let src_len = header.filesz();
                    assert_eq!(src.len(), src_len as usize);
                    let mut dst = unsafe { core::slice::from_raw_parts_mut(dst_addr as *mut u8, src_len as usize) };
                    for i in 0..src_len as usize {
                        dst[i] = src[i];
                    }
                }
            }
        }

        // add entrypoint offset
        let buf = unsafe { core::slice::from_raw_parts((kernel_tmp_p as u64 + 24) as *mut u8, 8)};
        let kernel_main_addr = LittleEndian::read_u64(&buf);

        // stop boot service
        bt.free_pool(kernel_tmp_p);
        system_table.exit_boot_services(_image_handler, &mut misc_buf).unwrap_success();

        // start kernel
        let kernel_main = unsafe {
            let f: extern "efiapi" fn() -> ! = core::mem::transmute(kernel_main_addr);
            f
        };
        kernel_main();
    }
    loop{};
    Status::SUCCESS;
}
