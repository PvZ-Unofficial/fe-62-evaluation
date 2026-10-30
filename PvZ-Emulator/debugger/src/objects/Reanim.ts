export interface Reanim {
    fps: number,
    prev_fps: number,
    begin_frame: number,
    n_frames: number,
    n_repeated: number,
    progress: number,
    prev_progress: number,
    type: "loop" |
        "loop_full_last_frame" |
        "play_once" |
        "play_once_and_hold" |
        "play_once_full_last_frame" |
        "play_once_full_last_frame_and_hold"
}
