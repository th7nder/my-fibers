use core::arch::asm;


// Stack size in bytes 
const SSIZE: isize = 48;

#[derive(Debug, Default)]
#[repr(C)]
/// x86-64 System-V ABI
struct ThreadContext {
    // stack pointer,
    rsp: u64
}

fn hello() -> ! {
    println!("just something, nothing to say");
    loop {}
}

unsafe fn gt_switch(new: *const ThreadContext) {
    asm!(
        "mov rsp, [{} + 0x00]",
        "ret",
        in(reg) new
    );
}


fn main() {
    let mut ctx = ThreadContext::default();
    let mut stack = vec![0_u8; SSIZE as usize];
    unsafe {
        let stack_bottom = stack.as_mut_ptr().offset(SSIZE);
        // Stack grows downwards, [0....48] -> 48 stack bottom
        // addr & ~0xF -> rounds down to the nearest 16-byte offset, and we know it'll be ours (as stack growns downard)
        // don't know what happens if we exhaust the stack though!
        let sb_aligned = (stack_bottom as usize & !0xF) as *mut u8;
        // last input, before call, must be 16 bytes aligned
        // if it's not on 16 byte, it'll try to get into not owned memory
        std::ptr::write(sb_aligned.offset(-16) as *mut u64, hello as u64);
        ctx.rsp = sb_aligned.offset(-16) as u64;
        gt_switch(&mut ctx);
    }
    // how does RSP work?
    println!("Hello, world!");
}
