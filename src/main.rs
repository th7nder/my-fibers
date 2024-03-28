#![feature(naked_functions)]
use std::arch::asm;

const DEFAULT_STACK_SIZE: usize = 1024 * 1024 * 2; // 2 MB
const MAX_THREADS: usize = 4;
static mut RUNTIME: usize = 0;


struct Runtime {
    threads: Vec<Thread>, 
    current: usize
}

impl Runtime {
    fn new() -> Runtime {
        let base_thread = Thread {
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            state: State::Running,
            context: ThreadContext::default()
        };

        let mut threads = vec![base_thread];
        let mut available_threads = (1..MAX_THREADS).map(|_| Thread::new()).collect();
        threads.append(&mut available_threads);

        Runtime {
            threads,
            current: 0
        }
    }

    fn init(&self) {
        unsafe {
            RUNTIME = self as *const Runtime as usize;
        }
    }
    fn run(&mut self) -> ! {
        while self.t_yield() {};
        std::process::exit(0)
    }

    fn spawn(&mut self, f: fn()) {
        let available_thread = self.threads
            .iter_mut()
            .find(|t| t.state == State::Available)
            .expect("there is no thread available");
        let stack_size = available_thread.stack.len();
        unsafe {
            // stacks grows downwards
            let s_ptr = available_thread.stack.as_mut_ptr().offset(stack_size as isize);
            // align to 16 byte border, as per System V abi
            let s_ptr = (s_ptr as usize & !0xF) as *mut u8;

            std::ptr::write(s_ptr.offset(-16) as *mut u64, guard as u64);
            std::ptr::write(s_ptr.offset(-24) as *mut u64, skip as u64);
            std::ptr::write(s_ptr.offset(-32) as *mut u64, f as u64);
            available_thread.context.rsp = s_ptr.offset(-32) as u64;
        }

        available_thread.state = State::Ready;
    }

    fn t_return(&mut self) {
        if self.current != 0 {
            self.threads[self.current].state = State::Available;
            self.t_yield();
        }
    }
    
    #[inline(never)]
    fn t_yield(&mut self) -> bool {
        let mut pos = self.current;

        while self.threads[pos].state != State::Ready {
            pos += 1;
            if pos == self.threads.len() {
                pos = 0;
            }

            if pos == self.current {
                // no threads are ready, quitting the runtime
                return false;
            }
        }

        if self.threads[self.current].state != State::Available {
            self.threads[self.current].state = State::Ready;
        }

        self.threads[pos].state = State::Running;
        let old = self.current;
        self.current = pos;

        let old_context: *mut ThreadContext = &mut self.threads[old].context;
        let new_context: *const ThreadContext = &mut self.threads[self.current].context;
        unsafe {
            asm!("call switch", in("rdi") old_context, in("rsi") new_context, clobber_abi("C"));
        }

        self.threads.len() > 0
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Available,
    Running,
    Ready,
}


struct Thread {
    stack: Vec<u8>,
    state: State,
    context: ThreadContext
}

impl Thread {
    fn new() -> Thread {
        Thread {
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            state: State::Available,
            context: ThreadContext::default(),
        }
    }
}

#[derive(Default, Debug)]
#[repr(C)]
struct ThreadContext {
    // x86-64 
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
}

fn guard() {
    unsafe {
        let rt = RUNTIME as *mut Runtime;
        (*rt).t_return();
    }
}

#[naked]
unsafe extern "C" fn skip() {
    asm!("ret", options(noreturn))
}

fn yield_thread() {
    unsafe {
        let rt = RUNTIME as *mut Runtime;
        (*rt).t_yield();
    }    
}

#[naked]
#[no_mangle]
#[cfg_attr(target_os = "macos", export_name = "\x01switch")]
unsafe extern "C" fn switch() {
    asm!(
        "mov [rdi + 0x00], rsp",
        "mov [rdi + 0x08], r15",
        "mov [rdi + 0x10], r14",
        "mov [rdi + 0x18], r13",
        "mov [rdi + 0x20], r12",
        "mov [rdi + 0x28], rbx",
        "mov [rdi + 0x30], rbp",
        "mov rsp, [rsi + 0x00]",
        "mov r15, [rsi + 0x08]",
        "mov r14, [rsi + 0x10]",
        "mov r13, [rsi + 0x18]",
        "mov r12, [rsi + 0x20]",
        "mov rbx, [rsi + 0x28]",
        "mov rbp, [rsi + 0x30]",
        "ret", options(noreturn)
    );
}


fn main() {
    let mut runtime = Runtime::new();
    runtime.init();
    runtime.spawn(|| {
        println!("Thread 1 starting");
        let id = 1;
        for i in 1..10 {
            println!("Hey from {id}, ha: {i}");
            yield_thread();
        }

        println!("Thread 1 stopped");
    });
    runtime.spawn(|| {
        println!("Thread 2 starting");
        let id = 2;
        for i in 1..10 {
            println!("Hey from {id}, ha: {i}");
            yield_thread();
        }

        println!("Thread 2 stopped");
    });

    runtime.run();
}

// errata:
// Implementing the runtime, before the t_yield implementation().
//  “If we find a thread that’s ready to be run, we change the state of the current thread from Running to Ready.”
//  I believe it should be from Ready to Running
// 
// Optimizing which code away, what does it mean? Which code could be optimized?