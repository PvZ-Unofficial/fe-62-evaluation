use rsvz_abi_macros::pvz_abi_fn;

use crate::raw::layout as ptrs;
use crate::raw::layout::{MouseWindow, SeedChooserScreen};

pvz_abi_fn! {
    // Native ProcessDeferredMessages callers 0x54EE0A..0x54EE7B and
    // callees 0x539390/0x5392E0/0x5394A0 verify these mixed carriers.
    widget_manager_mouse_down(manager:*mut MouseWindow,x:i32,y:i32,button:i32) {
        addr:0x539390, this:ecx=manager, regs:{eax=y,ebx=button}, stack:[x], clobber:[eax,ecx,edx],
    }
    widget_manager_mouse_up(manager:*mut MouseWindow,x:i32,y:i32,button:i32) {
        addr:0x5392e0, this:eax=manager, regs:{ebx=y}, stack:[button,x], clobber:[eax,ecx,edx],
    }
    widget_manager_mouse_move(manager:*mut MouseWindow,x:i32,y:i32) {
        addr:0x5394a0, this:edx=manager, regs:{eax=x,ecx=y}, clobber:[eax,ecx,edx],
    }
    // MessageWidget::SetLabel: ESI=widget, EDX=const native string&, ECX=style.
    // Native caller 0x40C96E..0x40C97D supplies these carriers; no stack args.
    message_widget_set_label(widget:*mut ptrs::MessageWidget, label:*const u32, style:i32) {
        addr:0x459010,
        this:esi=widget,
        regs:{edx=label,ecx=style},
        clobber:[eax,ecx,edx],
    }
    // widget_container_* / widget_manager_* / zombie_*

    widget_container_remove_widget(
        container: *mut MouseWindow,
        widget: *mut SeedChooserScreen,
    ) {
        addr: 0x5371c0,
        this: ecx = container,
        stack: [widget],
        clobber: [eax, ecx, edx],
    }

    // Objdump 0x54c74b..0x54c75e pushes LawnApp+0x320, calls 0x538eb0,
    // reads the bool result from AL, and 0x538eb0 returns with `ret 4`.
    widget_manager_draw_screen() -> u8 {
        addr: 0x538eb0,
        stack: [ptrs::LawnApp::mouse_window(ptrs::lawn_app())],
        clobber: [eax, ecx, edx],
        ret: al,
    }

    widget_manager_update_frame() {
        addr: 0x539140,
        this: edi = [0x6a9ec0, 0x320],
        clobber: [eax, ecx, edx],
    }
}
