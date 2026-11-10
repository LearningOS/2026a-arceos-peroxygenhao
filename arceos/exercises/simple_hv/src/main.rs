#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(asm_const)]
#![feature(riscv_ext_intrinsics)]

#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;
#[macro_use]
extern crate axlog;

mod task;
mod vcpu;
mod regs;
mod csrs;
mod sbi;
mod loader;

use vcpu::VmCpuRegisters;
use riscv::register::{scause, sstatus, stval};
use csrs::defs::hstatus;
use tock_registers::LocalRegisterCopy;
use csrs::{RiscvCsrTrait, CSR};
use vcpu::_run_guest;
use sbi::SbiMessage;
use loader::load_vm_image;
use axhal::mem::PhysAddr;
use crate::regs::GprIndex::{A0, A1};

const VM_ENTRY: usize = 0x8020_0000;

/// Guest physical address that the hypervisor exposes as an emulated device
/// register: `skernel2` loads its first SBI argument from here.
const GUEST_DEVICE_REG: usize = 0x40;

/// Value returned when the guest reads [`GUEST_DEVICE_REG`].
const GUEST_DEVICE_VALUE: usize = 0x6688;

/// Virtual hart id reported to the guest. The guest has no M-mode and therefore
/// no `mhartid` CSR, so the hypervisor answers on its behalf.
const GUEST_HART_ID: usize = 0x1234;

/// A RISC-V instruction is at least 2 bytes (compressed) and at most 4 bytes.
/// Skipping the trapping instruction needs its exact length, otherwise the
/// `sret` back into the guest re-executes the same instruction forever.
///
/// The guest image is loaded into freshly allocated frames, so a guest physical
/// address is *not* the same as the hypervisor's physical address and the
/// instruction cannot be peeked at through the linear mapping. The two
/// instructions the hypervisor emulates are fixed by `payload/skernel2`:
///
/// ```text
/// 80200000: f3 25 40 f1   csrr a1, mhartid
/// 80200004: 03 35 00 04   ld   a0, 0x40(zero)
/// ```
///
/// Both are 32-bit instructions (`llvm-objdump -d` on the release binary).
const GUEST_INSN_LEN: usize = 4;

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    ax_println!("Hypervisor ...");

    // A new address space for vm.
    let mut uspace = axmm::new_user_aspace().unwrap();

    // Load vm binary file into address space.
    if let Err(e) = load_vm_image("/sbin/skernel2", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // Setup context to prepare to enter guest mode.
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    // Setup pagetable for 2nd address mapping.
    let ept_root = uspace.page_table_root();
    prepare_vm_pgtable(ept_root);

    // Kick off vm and wait for it to exit.
    while !run_guest(&mut ctx) {
    }

    // The guest asked to be shut down and the hypervisor emulated the request,
    // so the whole machine can now go down normally (instead of panicking).
    ax_println!("Hypervisor exit [ok]!");
    axhal::misc::terminate();
}

fn prepare_vm_pgtable(ept_root: PhysAddr) {
    let hgatp = 8usize << 60 | usize::from(ept_root) >> 12;
    unsafe {
        core::arch::asm!(
            "csrw hgatp, {hgatp}",
            hgatp = in(reg) hgatp,
        );
        core::arch::riscv64::hfence_gvma_all();
    }
}

fn run_guest(ctx: &mut VmCpuRegisters) -> bool {
    unsafe {
        _run_guest(ctx);
    }

    vmexit_handler(ctx)
}

#[allow(unreachable_code)]
fn vmexit_handler(ctx: &mut VmCpuRegisters) -> bool {
    use scause::{Exception, Trap};

    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(Exception::VirtualSupervisorEnvCall) => {
            let sbi_msg = SbiMessage::from_regs(ctx.guest_regs.gprs.a_regs()).ok();
            ax_println!("VmExit Reason: VSuperEcall: {:?}", sbi_msg);
            if let Some(msg) = sbi_msg {
                match msg {
                    SbiMessage::Reset(_) => {
                        let a0 = ctx.guest_regs.gprs.reg(A0);
                        let a1 = ctx.guest_regs.gprs.reg(A1);
                        ax_println!("a0 = {:#x}, a1 = {:#x}", a0, a1);
                        assert_eq!(a0, 0x6688);
                        assert_eq!(a1, 0x1234);
                        ax_println!("Shutdown vm normally!");
                        return true;
                    },
                    _ => todo!(),
                }
            } else {
                panic!("bad sbi message! ");
            }
        },
        Trap::Exception(Exception::IllegalInstruction) => {
            // First VM-exit reason: the guest executed an instruction that does
            // not exist in VS-mode. `skernel2` reads `mhartid`, a machine-level
            // CSR, because it expects to run on bare metal. Emulate the read by
            // answering a virtual hart id and resume after the instruction.
            let pc = ctx.guest_regs.sepc;
            ax_println!(
                "VmExit Reason: IllegalInstruction at {:#x}, emulate csrr mhartid",
                pc
            );
            ctx.guest_regs.gprs.set_reg(A1, GUEST_HART_ID);
            ctx.guest_regs.sepc = pc + GUEST_INSN_LEN;
            return false;
        },
        Trap::Exception(Exception::LoadGuestPageFault) => {
            // Second VM-exit reason: the guest touched a guest physical address
            // that has no second-stage mapping. That address is an emulated
            // device register, so answer the load and resume after it.
            let faulting_addr = stval::read();
            if faulting_addr != GUEST_DEVICE_REG {
                panic!(
                    "LoadGuestPageFault at {:#x}: stval {:#x} sepc: {:#x}",
                    faulting_addr,
                    faulting_addr,
                    ctx.guest_regs.sepc
                );
            }
            let pc = ctx.guest_regs.sepc;
            ax_println!(
                "VmExit Reason: LoadGuestPageFault at {:#x}, emulate device register {:#x}",
                pc,
                GUEST_DEVICE_REG
            );
            ctx.guest_regs.gprs.set_reg(A0, GUEST_DEVICE_VALUE);
            ctx.guest_regs.sepc = pc + GUEST_INSN_LEN;
            return false;
        },
        _ => {
            panic!(
                "Unhandled trap: {:?}, sepc: {:#x}, stval: {:#x}",
                scause.cause(),
                ctx.guest_regs.sepc,
                stval::read()
            );
        }
    }
    false
}

fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
    // Set hstatus
    let mut hstatus = LocalRegisterCopy::<usize, hstatus::Register>::new(
        riscv::register::hstatus::read().bits(),
    );
    // Set Guest bit in order to return to guest mode.
    hstatus.modify(hstatus::spv::Guest);
    // Set SPVP bit in order to accessing VS-mode memory from HS-mode.
    hstatus.modify(hstatus::spvp::Supervisor);
    CSR.hstatus.write_value(hstatus.get());
    ctx.guest_regs.hstatus = hstatus.get();

    // Set sstatus in guest mode.
    let mut sstatus = sstatus::read();
    sstatus.set_spp(sstatus::SPP::Supervisor);
    ctx.guest_regs.sstatus = sstatus.bits();
    // Return to entry to start vm.
    ctx.guest_regs.sepc = VM_ENTRY;
}
