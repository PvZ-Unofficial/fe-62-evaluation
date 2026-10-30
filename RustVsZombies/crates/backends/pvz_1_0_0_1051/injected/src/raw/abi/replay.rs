use crate::raw::layout::{Board, LawnApp};
use rsvz_abi_macros::pvz_abi_fn;

pvz_abi_fn! {
    // SyncReanimationDef 4814F9..481507: push preload=1, push kind,
    // call 4735E0, caller adds 8. Function returns with plain RET at 473749.
    replay_ensure_animation(kind:i32,preload:u32) {
        addr:0x4735e0, stack:[kind,preload], cleanup:8, clobber:[eax,ecx,edx],
    }

    // LawnSaveGame 4820F0/4820F4: EAX=&Buffer; ctor returns with plain ret.
    replay_buffer_new(buffer:*mut u32) { addr:0x5d60a0,this:eax=buffer,clobber:[eax,ecx,edx], }
    // LawnSaveGame 48216D: ECX=&Buffer; dtor frees its native allocator block.
    replay_buffer_drop(buffer:*mut u32) { addr:0x5d60c0,this:ecx=buffer,clobber:[eax,ecx,edx], }
    // Native header call 482110..48212D: ESI=12, EAX=header, ECX=context.
    replay_sync_header(context:*mut u32,header:*mut u32) {addr:0x4813d0,this:ecx=context,regs:{eax=header,esi=12},clobber:[eax,ecx,edx],}
    // Native 482132..48213C: EAX=context, push Board, caller adds four bytes.
    replay_sync_board(context:*mut u32,board:*mut Board) {addr:0x4819d0,this:eax=context,stack:[board],cleanup:4,clobber:[eax,ecx,edx],}
    // 46EB23 copies EAX=Projectile; plain ret 46EB41/46EBB9; caller 46D047 puts it in EAX.
    scene_projectile_die(projectile:*mut crate::raw::layout::Projectile) {
        addr:0x46eb20, this:eax=projectile, clobber:[eax,ecx,edx],
    }
    // 432DD0 writes Coin+38 using ESI; plain ret 432E19; caller 4309D1 puts the coin in ESI.
    scene_coin_die(coin:*mut crate::raw::layout::Coin) {
        addr:0x432dd0, this:esi=coin, clobber:[eax,ecx,edx],
    }
    // 408DE0 pushes EAX(path), ECX=EDI(Board), then calls LawnLoadGame.
    // AL boolean, plain ret 408DF1/408E33; success reloads scene resources. Caller 44F804..44F813 supplies EDI/EAX.
    replay_load(board:*mut Board,path:*const u32)->u8 {
        addr:0x408de0, this:edi=board, regs:{eax=path},
        clobber:[eax,ecx,edx], ret:al,
    }
    // 44F607 copies ECX(App) to EDI, calls KillBoard, allocates/attaches Board.
    replay_new_board(app:*mut LawnApp) {
        addr:0x44f5f0, this:ecx=app, clobber:[eax,ecx,edx],
    }
    // 551921 reads App from original stack+4; native ret 4 at 5519E6.
    replay_safe_delete(app:*mut LawnApp) {
        addr:0x5518f0, stack:[app], clobber:[eax,ecx,edx],
    }
}
