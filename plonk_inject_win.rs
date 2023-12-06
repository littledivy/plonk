/*
 * Copyright (c) 2023 Divy Srivastava
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

/* DLL injector for Windows */

use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::Command;

use dynasmrt::dynasm;
use dynasmrt::DynasmApi;
use dynasmrt::DynasmLabelApi;
use winapi::shared::minwindef::HMODULE;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;
use winapi::um::winbase::WAIT_FAILED;

/// This uses the `CreateRemoteThread` technique to inject a DLL into the process.
pub unsafe fn inject(command: &mut Command, dll_path: &str) {
    let mut process = command.spawn().unwrap();
    let process_handle = process.as_raw_handle();

    let k32 = winapi::um::libloaderapi::GetModuleHandleA("kernel32.dll\0".as_ptr() as *const i8);
    if k32.is_null() {
        panic!("Failed to get kernel32.dll handle");
    }

    let loadlib =
        winapi::um::libloaderapi::GetProcAddress(k32, "LoadLibraryW\0".as_ptr() as *const i8)
            as usize;
    if loadlib == 0 {
        panic!("Failed to get LoadLibraryA address");
    }

    let get_last_error =
        winapi::um::libloaderapi::GetProcAddress(k32, "GetLastError\0".as_ptr() as *const i8)
            as usize;
    if get_last_error == 0 {
        panic!("Failed to get GetLastError address");
    }

    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    let hmodule = alloc_remote(process_handle, size_of::<HMODULE>()).unwrap() as usize;
    dynasm!(ops
        ; .arch x64
        ; sub rsp, 40
        ; mov rax, QWORD loadlib as i64
        ; call rax
        ; movabs hmodule as i64, eax
    );

    let label = ops.new_dynamic_label();
    dynasm!(ops
        ; .arch x64
        ; test rax, rax
        ; mov rax, 0
        ; jnz =>label
        ; mov rax, QWORD get_last_error as i64
        ; call rax
    );
    ops.dynamic_label(label);

    dynasm!(ops
        ; .arch x64
        ; add rsp, 40
        ; ret
    );

    let code = ops.finalize().unwrap();

    println!("Code compiled");
    let code_alloc = alloc_remote(process_handle, code.len()).unwrap();

    write_process_memory(process_handle, code_alloc, &code).unwrap();

    let wide_dll_path = std::ffi::OsStr::new(dll_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let wide_dll_byte_slice = std::slice::from_raw_parts(
        wide_dll_path.as_ptr() as *const u8,
        wide_dll_path.len() * size_of::<u16>(),
    );
    let parameter = alloc_remote(process_handle, wide_dll_byte_slice.len()).unwrap();
    write_process_memory(process_handle, parameter, wide_dll_byte_slice).unwrap();

    let thread_handle = winapi::um::processthreadsapi::CreateRemoteThread(
        process_handle,
        std::ptr::null_mut(),
        0,
        Some(std::mem::transmute(code_alloc)),
        parameter as _,
        0,
        std::ptr::null_mut(),
    );

    if thread_handle.is_null() {
        panic!("Failed to create remote thread");
    }

    let reason = WaitForSingleObject(thread_handle, INFINITE);
    if reason == WAIT_FAILED {
        println!("{}", std::io::Error::last_os_error());
        panic!("Failed to wait for remote thread");
    }

    let mut exit_code = std::mem::MaybeUninit::uninit();
    let result = unsafe {
        winapi::um::processthreadsapi::GetExitCodeThread(thread_handle, exit_code.as_mut_ptr())
    };

    if result == 0 {
        panic!("Failed to get exit code of remote thread");
    }
    debug_assert_ne!(
        result as u32,
        winapi::um::minwinbase::STILL_ACTIVE,
        "GetExitCodeThread returned STILL_ACTIVE after WaitForSingleObject"
    );

    let exit_code = unsafe { exit_code.assume_init() };

    if exit_code != 0 {
        if exit_code == 0xc0000005 {
            println!("Exit code: (Access violation)");
        } else {
            println!("{}", std::io::Error::from_raw_os_error(exit_code as i32));
        }
    }

    process.wait().unwrap();
}

fn alloc_remote(
    process_handle: winapi::um::winnt::HANDLE,
    size: usize,
) -> Result<*mut std::ffi::c_void, ()> {
    let address = unsafe {
        winapi::um::memoryapi::VirtualAllocEx(
            process_handle,
            std::ptr::null_mut(),
            size,
            winapi::um::winnt::MEM_COMMIT | winapi::um::winnt::MEM_RESERVE,
            winapi::um::winnt::PAGE_EXECUTE_READWRITE,
        )
    };

    if address.is_null() {
        return Err(());
    }

    Ok(address)
}

fn write_process_memory(
    process_handle: winapi::um::winnt::HANDLE,
    address: *mut std::ffi::c_void,
    data: &[u8],
) -> Result<(), std::io::Error> {
    let mut bytes_written = 0;
    let result = unsafe {
        winapi::um::memoryapi::WriteProcessMemory(
            process_handle,
            address,
            data.as_ptr() as *const std::ffi::c_void,
            data.len(),
            &mut bytes_written,
        )
    };

    if result == 0 {
        return Err(std::io::Error::last_os_error());
    }

    assert!(bytes_written == data.len());

    Ok(())
}
