#include <cmath>
#include <algorithm>

#include "rapidjson/writer.h"
#include "rapidjson/stringbuffer.h"

#include "common.h"
#include "scene.h"
#include "griditem.h"

namespace pvz_emulator::object {

bool rect::is_overlap_with_circle(int px, int py, int r) {
    bool x_in_range = x <= px && px <= x + width;
    bool y_in_range = y <= py && py <= y + height;

    if (x_in_range && y_in_range) {
        return true;
    } else if (x_in_range) {
        return std::min(abs(py - y), abs(py - (y + height))) <= r;
    } else if (y_in_range) {
        return std::min(abs(px - x), abs(px - (x + width))) <= r;
    } else {
        int xl = std::min(abs(px - x), abs(px - (x + width)));
        int yl = std::min(abs(py - y), abs(py - (y + height)));

        return pow(xl, 2) + pow(yl, 2) <= pow(r, 2);
    }
}

float rect::get_overlap_len(const rect& r) {
    float near_l, far_l, far_x;
    
    if (r.x >= this->x) {
        near_l = static_cast<float>(this->x + this->width);
        far_l = static_cast<float>(r.x + r.width);
        far_x = static_cast<float>(r.x);
    } else {
        near_l = static_cast<float>(r.x + r.width);
        far_l = static_cast<float>(this->x + this->width);
        far_x = static_cast<float>(this->x);
    }

    if (near_l > far_x && near_l > far_l) {
        return far_l - far_x;
    } else {
        return near_l - far_x;
    }
}

void rect::to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer) {
    writer.StartObject();
    writer.Key("x");
    writer.Int(x);
    writer.Key("y");
    writer.Int(y);
    writer.Key("width");
    writer.Int(width);
    writer.Key("height");
    writer.Int(height);
    writer.EndObject();
}

bool reanim::is_in_progress(float p) {
    if (prev_progress <= progress) {
        return prev_progress <= p && p < progress;
    } else {
        return prev_progress <= p || p < progress;
    }
}

void reanim::to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer) {
    writer.StartObject();

    writer.Key("fps");
    writer.Double(fps);

    writer.Key("prev_fps");
    writer.Double(prev_fps);

    writer.Key("begin_frame");
    writer.Uint(begin_frame);

    writer.Key("n_frames");
    writer.Uint(n_frames);

    writer.Key("n_repeated");
    writer.Uint(n_repeated);

    writer.Key("progress");
    writer.Double(progress);

    writer.Key("prev_progress");
    writer.Double(prev_progress);

    writer.Key("type");
    switch (type) {
    case reanim_type::loop:
        writer.String("loop");
        break;
    case reanim_type::loop_full_last_frame:
        writer.String("loop_full_last_frame");
        break;
    case reanim_type::play_once:
        writer.String("play_once");
        break;
    case reanim_type::play_once_and_hold:
        writer.String("play_once_and_hold");
        break;
    case reanim_type::play_once_full_last_frame:
        writer.String("play_once_full_last_frame");
        break;
    case reanim_type::play_once_full_last_frame_and_hold:
        writer.String("play_once_full_last_frame_and_hold");
        break;
    }

    writer.EndObject();
}

void griditem::to_json(
    scene& scene,
    rapidjson::Writer<rapidjson::StringBuffer>& writer)
{
    writer.StartObject();
    writer.Key("id");
    writer.Uint64(scene.griditems.get_id(*this));

    writer.Key("type");
    switch (type) {
    case griditem_type::grave:
        writer.String("grave");
        break;
    case griditem_type::crater:
        writer.String("crater");
        break;
    case griditem_type::ladder:
        writer.String("ladder");
        break;
    default:
        writer.Null();
    }

    writer.Key("col");
    writer.Uint(col);

    writer.Key("row");
    writer.Uint(row);

    writer.Key("countdown");
    writer.Int(countdown);

    writer.Key("is_disappeared");
    writer.Bool(is_disappeared);

    writer.EndObject();
}

void reanim::get_frame_status(reanim_frame_status& rfs) const {
    float current_frame = static_cast<float>(
        static_cast<double>(progress) * (n_frames - 1) + begin_frame);

    double floored_current_frame = floor(current_frame);

    rfs.frame_progress = static_cast<float>(current_frame - floored_current_frame);

    floored_current_frame = round(floored_current_frame);

    int end_frame = begin_frame + n_frames - 1;

    if (floored_current_frame < end_frame) {
        rfs.frame = static_cast<unsigned int>(floored_current_frame);
        rfs.next_frame = rfs.frame + 1;
    } else {
        rfs.frame = end_frame;
        rfs.next_frame = end_frame;
    }
}

}
