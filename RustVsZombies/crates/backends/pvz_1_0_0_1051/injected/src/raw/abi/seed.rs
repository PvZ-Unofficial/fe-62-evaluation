use rsvz_abi_macros::pvz_abi_fn;

use crate::raw::layout::{ChosenSeed, SeedPacket};

use crate::raw::layout as ptrs;

pvz_abi_fn! {
    // 0x485E90: EAX chooser, stack ChosenSeed*, preserves EBX/ESI/EDI/EBP,
    // ret 4 at 0x486028. MouseDown caller 0x486C96..0x486C9E.
    seed_chooser_clicked_seed_in_bank(chosen_seed: *mut ChosenSeed) {
        addr: 0x485e90,
        this: eax = [0x6a9ec0, 0x774],
        stack: [chosen_seed],
        clobber: [eax, ecx, edx],
    }
    // Native 0x453B20 and callers including 0x402217..0x40221B.
    lawn_app_has_seed_type(kind: i32) -> u8 {
        addr: 0x453b20,
        this: edi = [0x6a9ec0],
        regs: { ebx = kind },
        clobber: [eax, ecx, edx],
        ret: al,
    }
    // EAX chooser, EBX seed type; plain ret. Caller 0x4863AD..0x4863B1.
    seed_not_allowed_during_trial(kind: i32) -> u8 {
        addr: 0x484640,
        this: eax = [0x6a9ec0, 0x774],
        regs: { ebx = kind },
        clobber: [eax, ecx, edx],
        ret: al,
    }
    // Verified 0x489CD0: ESI=bank, EAX=count when GetNumSeedsInBank is bypassed;
    // EDI/EBX are saved by the callee; no stack arguments or callee cleanup.
    seed_bank_update_width_with_count(bank: *mut ptrs::SeedBank, count: i32) {
        addr: 0x489cd0,
        this: esi = bank,
        regs: { eax = count },
        clobber: [eax, ecx, edx],
    }
    // seed_* / seed_chooser_* / seed_packet_*

    seed_bank_refresh_all_packets() {
        addr: 0x489d50,
        stack: [ptrs::Board::seed_bank(ptrs::board())],
        clobber: [eax, ecx, edx],
    }

    seed_chooser_clicked_seed_in_chooser(chosen_seed: *mut ChosenSeed) {
        addr: 0x486030,
        this: eax = [0x6a9ec0, 0x774],
        stack: [chosen_seed],
        clobber: [eax, ecx, edx],
    }

    seed_chooser_close_seed_chooser() {
        addr: 0x486d20,
        this: ebx = [0x6a9ec0, 0x774],
        clobber: [eax, ecx, edx],
    }

    seed_chooser_pick_random_seeds() {
        addr: 0x4859b0,
        stack: [ptrs::LawnApp::seed_chooser(ptrs::lawn_app())],
        clobber: [eax, ecx, edx],
    }

    seed_chooser_update_imitater_button() {
        addr: 0x4866e0,
        stack: [ptrs::LawnApp::seed_chooser(ptrs::lawn_app())],
        clobber: [eax, ecx, edx],
    }

    seed_packet_can_pick_up(seed_packet: *mut SeedPacket) -> u8 {
        addr: 0x488500,
        this: esi = seed_packet,
        clobber: [eax, edx],
        ret: al,
    }

    // Objdump 0x00488EC0: reads SeedPacket* from eax and returns normally without stack cleanup.
    seed_packet_was_planted(seed_packet: *mut SeedPacket) {
        addr: 0x488ec0,
        this: eax = seed_packet,
        clobber: [eax, ecx, edx],
    }
}
