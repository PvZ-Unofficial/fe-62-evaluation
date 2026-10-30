#pragma once
#include "rapidjson/writer.h"
#include "rapidjson/stringbuffer.h"

namespace pvz_emulator::object {

struct rect {
    int x;
    int y;
    int width;
    int height;

    bool is_overlap_with_circle(int px, int py, int r);

    float get_overlap_len(const rect& r);

    void to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer);
};

enum class reanim_type {
    loop = 0,
    loop_full_last_frame = 1,
    play_once = 2,
    play_once_and_hold = 3,
    play_once_full_last_frame = 4,
    play_once_full_last_frame_and_hold = 5,

    repeat = loop,
    once = play_once_and_hold,
};

struct reanim_frame_status {
    float frame_progress;
    unsigned int frame;
    unsigned int next_frame;
};

struct reanim {
    float fps;
    float prev_fps;
    unsigned int begin_frame;
    unsigned int n_frames;
    unsigned int n_repeated;
    float progress;
    float prev_progress;
    reanim_type type;

    bool is_in_progress(float progress);

    void get_frame_status(reanim_frame_status& rfs) const;

    void to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer);
};

}
