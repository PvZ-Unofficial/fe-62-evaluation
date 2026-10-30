#pragma once

#include "raw_layout.h"
#include "PvzpLib/Plugin.h"

#include <cstddef>
#include <cstdint>
#include <stdint.h>

#define PVZP_RS_EXPORT

extern "C" {
void pvzp_rs_set_key_sink(void (*callback)(std::uint32_t)) noexcept;
void pvzp_rs_set_board_click_capture(bool enabled) noexcept;
void pvzp_rs_set_board_click_native_priority(bool enabled) noexcept;
bool pvzp_rs_board_click_capture_active() noexcept;
void pvzp_rs_set_lawn_preview(int kind, bool imitate) noexcept;
int pvzp_rs_take_board_click(int* x,int* y,int* button) noexcept;
void pvzp_rs_paint_source(std::uint32_t source) noexcept;
void pvzp_rs_paint_clear_source(std::uint32_t source) noexcept;
std::uint32_t pvzp_rs_paint_create_image(const uint32_t* pixels, int width, int height) noexcept;
std::uint8_t pvzp_rs_paint_image(std::uint32_t id, int x, int y, std::uint32_t argb) noexcept;
std::uint8_t pvzp_rs_replay_save(std::uintptr_t file) noexcept;
std::uint8_t pvzp_rs_replay_capture(void* context, std::uint8_t (*receive)(void*, const unsigned char*, std::uint64_t)) noexcept;
std::uint8_t pvzp_rs_replay_restore_bytes(const unsigned char* bytes, std::size_t length, std::int32_t mode) noexcept;
std::uint8_t pvzp_rs_replay_restore(const char* path, std::int32_t mode) noexcept;

std::uint8_t pvzp_rs_set_paint(void (*callback)()) noexcept;
std::uint8_t pvzp_rs_paint_grid_row(int* row) noexcept;
std::uint8_t pvzp_rs_paint_pointer(std::uint8_t sprite,int* x,int* y) noexcept;
std::uint8_t pvzp_rs_paint_size(std::uint32_t* width, std::uint32_t* height) noexcept;
std::uint8_t pvzp_rs_paint_cursor(int x,int y,std::uint8_t hand) noexcept;
std::uint8_t pvzp_rs_paint_rect(int x, int y, int width, int height, std::uint32_t argb) noexcept;
std::uint8_t pvzp_rs_paint_clip(std::uint8_t enabled, int x, int y, int width, int height) noexcept;
std::uint8_t pvzp_rs_paint_text(int x, int y, const char* text, int length, const uint16_t* family,
    int pixels, std::uint8_t bold, std::uint32_t argb, std::uint8_t measure, std::uint32_t* width, std::uint32_t* height) noexcept;
std::uint8_t pvzp_rs_paint_digit_bounds(const uint16_t* family, int pixels, std::uint8_t bold,
    int* top, int* bottom) noexcept;

// Checked against Board's native array extent when compiling the bridge.
enum pvzp_rs_spawn_bounds : std::uint32_t {
    PVZP_RS_SPAWN_WAVE_CAPACITY = 100,
};

std::int32_t pvzp_rs_seed_base_cost(std::int32_t kind) noexcept;
std::int32_t pvzp_rs_seed_base_recharge(std::int32_t kind) noexcept;

using pvzp_rs_battle_callbacks = PvzpPlugin::BattleCallbacks;
using pvzp_rs_layout_entry = PvzpPlugin::LayoutEntry;
std::uint8_t pvzp_rs_check_raw_layout(const pvzp_rs_layout_entry* entries, std::uint32_t count) noexcept;
std::uint8_t pvzp_rs_validate_abi() noexcept;
std::uint8_t pvzp_rs_register_update(PvzpPlugin::UpdateCallback update, PvzpPlugin::BoardDestroyingCallback destroy) noexcept;
std::uint8_t pvzp_rs_register_battle(const pvzp_rs_battle_callbacks* callbacks, std::uint32_t interest) noexcept;
void pvzp_rs_clear_battle() noexcept;
void pvzp_rs_request_stop() noexcept;

struct pvzp_rs_world;
void pvzp_rs_replay_clock(pvzp_rs_world*, std::uint32_t, std::int32_t) noexcept;
void pvzp_rs_replay_digger_facing(pvzp_rs_reanimation*, float, bool) noexcept;
void pvzp_rs_replay_gargantuar_damage(pvzp_rs_zombie*) noexcept;
void pvzp_rs_replay_plant_damage(pvzp_rs_plant*) noexcept;
void pvzp_rs_replay_ice(pvzp_rs_world*, std::uint32_t, std::int32_t, std::int32_t) noexcept;
pvzp_rs_projectile* pvzp_rs_replay_projectile(pvzp_rs_world*, int, int, int, int) noexcept;
struct pvzp_rs_plant_pool;
struct pvzp_rs_zombie_pool;
struct pvzp_rs_projectile_pool;
struct pvzp_rs_grid_item_pool;
struct pvzp_rs_item_pool;
struct pvzp_rs_mower_pool;

enum pvzp_rs_status_code : std::int32_t {
	PVZP_RS_STATUS_OK = 0,
	PVZP_RS_STATUS_NULL_POINTER = 1,
	PVZP_RS_STATUS_INVALID_ARGUMENT = 2,
	PVZP_RS_STATUS_NOT_FOUND = 3,
	PVZP_RS_STATUS_UNSUPPORTED = 4,
	PVZP_RS_STATUS_STD_EXCEPTION = 100,
	PVZP_RS_STATUS_BAD_ALLOC = 101,
	PVZP_RS_STATUS_UNKNOWN_EXCEPTION = 102,
};

struct pvzp_rs_status {
	std::int32_t code;
	std::int32_t detail;
};

struct pvzp_rs_rect_i32 {
	std::int32_t x;
	std::int32_t y;
	std::int32_t width;
	std::int32_t height;
};






#define PVZP_RS_API PVZP_RS_EXPORT pvzp_rs_status

PVZP_RS_API pvzp_rs_set_common_dance(std::int32_t dance) noexcept;
PVZP_RS_API pvzp_rs_set_dance_mode(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_set_cob_impact_delay(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_coins(std::uint32_t* value) noexcept;
PVZP_RS_API pvzp_rs_set_coins(std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_pickup_limit(std::uint8_t money, std::uint32_t* value) noexcept;
PVZP_RS_API pvzp_rs_set_pickup_limit(std::uint8_t money, std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_profile_writable() noexcept;
PVZP_RS_API pvzp_rs_finished_adventure(std::uint32_t* value) noexcept;
PVZP_RS_API pvzp_rs_set_finished_adventure(std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_challenge_record(std::uint32_t index, std::uint32_t* value) noexcept;
PVZP_RS_API pvzp_rs_set_challenge_record(std::uint32_t index, std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_purchase(std::uint32_t index, std::uint32_t* value) noexcept;
PVZP_RS_API pvzp_rs_set_purchase(std::uint32_t index, std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_sync_game_selector(std::uint8_t loading) noexcept;
PVZP_RS_API pvzp_rs_mouse(std::int32_t operation, std::int32_t x, std::int32_t y, std::int32_t button) noexcept;
PVZP_RS_API pvzp_rs_sound(std::uint32_t id, std::uint8_t stop) noexcept;
PVZP_RS_API pvzp_rs_water_plant(std::int32_t row, std::int32_t col) noexcept;
PVZP_RS_API pvzp_rs_unlock_trophy() noexcept;
PVZP_RS_API pvzp_rs_unlock_hidden_modes() noexcept;
PVZP_RS_API pvzp_rs_world_generation(std::uint64_t* generation) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_temporary_unlock_enabled() noexcept;
PVZP_RS_API pvzp_rs_set_temporary_unlock_enabled(std::uint8_t enabled) noexcept;

// Each object must be an occupied slot kept alive by its backend borrow.
PVZP_RS_EXPORT std::uint32_t pvzp_rs_plant_id(const pvzp_rs_plant* object) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_zombie_id(const pvzp_rs_zombie* object) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_grid_item_id(const pvzp_rs_grid_item* object) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_projectile_id(const pvzp_rs_projectile* object) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_item_id(const pvzp_rs_item* object) noexcept;

PVZP_RS_API pvzp_rs_current_world(pvzp_rs_world** out_world) noexcept;
// Requires live gLawnApp on the game thread. Board may be null.
PVZP_RS_EXPORT pvzp_rs_world* pvzp_rs_board_from_live_app() noexcept;
PVZP_RS_API pvzp_rs_game_ui(std::int32_t* out_game_ui) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_mouse_visible(std::uint8_t* out_visible) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_parent_present(std::uint8_t* out_present) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_widget_manager_present(std::uint8_t* out_present) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_modal_present(std::uint8_t* out_present) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_choose_state(std::int32_t* out_state) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_view_lawn_time(std::int32_t* out_time) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_seeds_in_flight(std::int32_t* out_count) noexcept;
PVZP_RS_API pvzp_rs_seed_chooser_cancel_view_lawn() noexcept;
PVZP_RS_API pvzp_rs_click_continue_dialog_if_present(std::uint8_t* out_clicked) noexcept;
extern "C" std::uint8_t pvzp_rs_key_is_down(std::uint16_t key) noexcept;
PVZP_RS_API pvzp_rs_input_focused(std::uint8_t* out_focused) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_scene(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_world_paused(const pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_seed_choosing(const pvzp_rs_world* world, std::uint8_t* out_choosing) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_world_main_counter(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_world_imitater_successor(const pvzp_rs_world* world, std::uint32_t placeholder_id, std::uint32_t* out_successor_id) noexcept;
PVZP_RS_API pvzp_rs_world_cursor_type(const pvzp_rs_world* world, std::int32_t* out_cursor_type) noexcept;
PVZP_RS_API pvzp_rs_select_cob(pvzp_rs_world* world, std::uint32_t id) noexcept;
PVZP_RS_API pvzp_rs_new_plant_native_grid(pvzp_rs_world* world, std::int32_t x, std::int32_t y, std::int32_t kind, std::int32_t imitater, pvzp_rs_plant** out) noexcept;
PVZP_RS_API pvzp_rs_board_view(const pvzp_rs_world*,std::int32_t*,std::int32_t*) noexcept;
PVZP_RS_API pvzp_rs_set_board_view(pvzp_rs_world*,std::int32_t,std::int32_t) noexcept;
PVZP_RS_API pvzp_rs_animation_skip(std::uint8_t*) noexcept;
PVZP_RS_API pvzp_rs_set_animation_skip(std::uint8_t) noexcept;
PVZP_RS_API pvzp_rs_house_pause(std::uint8_t*) noexcept;
PVZP_RS_API pvzp_rs_set_house_pause(std::uint8_t) noexcept;
PVZP_RS_API pvzp_rs_take_house(std::uint8_t*) noexcept;
PVZP_RS_API pvzp_rs_clear_fast_mode() noexcept;
PVZP_RS_API pvzp_rs_scene_music(std::int32_t) noexcept;
PVZP_RS_API pvzp_rs_cursor_position(const pvzp_rs_world* world, std::int32_t* x, std::int32_t* y) noexcept;
PVZP_RS_API pvzp_rs_cursor_object_position(const pvzp_rs_world* world, std::int32_t* x, std::int32_t* y) noexcept;
PVZP_RS_API pvzp_rs_cursor_grid_row(const pvzp_rs_world* world, std::int32_t* value) noexcept;
PVZP_RS_API pvzp_rs_cursor_seed_slot(const pvzp_rs_world* world, std::int32_t* value) noexcept;
PVZP_RS_API pvzp_rs_world_cursor_seed_kind(const pvzp_rs_world* world, std::int32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_cursor_imitater_kind(const pvzp_rs_world* world, std::int32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_has_conveyor(const pvzp_rs_world* world, std::uint8_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_is_column_challenge(const pvzp_rs_world* world, std::uint8_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_is_wallnut_bowling(const pvzp_rs_world* world, std::uint8_t* out_value) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_current_wave(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_sun(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_world_level_complete(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_next_survival_stage_counter(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_frame_duration_ms(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_survival_stage(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_survival_flags_completed(const pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_set_survival_stage(pvzp_rs_world* world, std::int32_t stage) noexcept;
PVZP_RS_API pvzp_rs_world_set_pause_dialog(pvzp_rs_world* world, std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_world_begin_level_completion(pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_validate_level_completion(const pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_show_hint(pvzp_rs_world* world, const char* text, std::uint32_t length, std::int32_t duration) noexcept;
PVZP_RS_API pvzp_rs_world_caption_text(const pvzp_rs_world* world, char* output, std::uint32_t capacity, std::uint32_t* length) noexcept;
PVZP_RS_API pvzp_rs_world_caption_duration(const pvzp_rs_world* world, std::int32_t* value) noexcept;
PVZP_RS_API pvzp_rs_world_caption_style(const pvzp_rs_world* world, std::int32_t* value) noexcept;
PVZP_RS_API pvzp_rs_world_set_sun(pvzp_rs_world* world, std::uint32_t sun) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_natural_sun_generated(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_natural_sun_countdown(const pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_set_natural_sun_generated(pvzp_rs_world* world, std::int32_t count) noexcept;
PVZP_RS_API pvzp_rs_world_set_natural_sun_countdown(pvzp_rs_world* world, std::int32_t countdown) noexcept;
PVZP_RS_API pvzp_rs_app_counter(std::uint32_t* out_counter) noexcept;
PVZP_RS_API pvzp_rs_set_app_counter(std::uint32_t counter) noexcept;
PVZP_RS_API pvzp_rs_world_ice_path_x(const pvzp_rs_world* world, std::uint32_t row, std::int32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_ice_path_countdown(const pvzp_rs_world* world, std::uint32_t row, std::uint32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_row_pick_weight_bits(const pvzp_rs_world* world, std::uint32_t row, std::uint32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_row_pick_last_picked_bits(const pvzp_rs_world* world, std::uint32_t row, std::uint32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_row_pick_second_last_picked_bits(const pvzp_rs_world* world, std::uint32_t row, std::uint32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_world_spawn_allowed(const pvzp_rs_world* world, std::uint32_t zombie_type, std::uint8_t* out_allowed) noexcept;
PVZP_RS_API pvzp_rs_world_spawn_entry(const pvzp_rs_world* world, std::uint32_t wave, std::uint32_t slot, std::int32_t* out_zombie_type) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_total_waves(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_refresh_countdown(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_initial_countdown(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_huge_wave_countdown(const pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT std::int32_t pvzp_rs_world_level_end_countdown(const pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_commit_wave_refresh(pvzp_rs_world* world, std::int32_t expected_wave, std::int32_t initial_countdown) noexcept;
PVZP_RS_API pvzp_rs_world_zombie_refresh_health_threshold(const pvzp_rs_world* world, std::int32_t* out_health) noexcept;
std::int32_t pvzp_rs_zombie_hurt_width(const pvzp_rs_zombie* zombie) noexcept;
std::int32_t pvzp_rs_zombie_hurt_height(const pvzp_rs_zombie* zombie) noexcept;
PVZP_RS_API pvzp_rs_hud_read(std::uint32_t field,std::int32_t* value) noexcept;
PVZP_RS_API pvzp_rs_hud_write(std::uint32_t field,std::int32_t value) noexcept;
PVZP_RS_API pvzp_rs_world_zombie_health_wave_start(const pvzp_rs_world* world, std::int32_t* out_health) noexcept;
PVZP_RS_API pvzp_rs_world_total_zombie_health_in_wave(pvzp_rs_world* world, std::int32_t wave, std::int32_t* out_health) noexcept;
PVZP_RS_API pvzp_rs_world_set_scene(pvzp_rs_world* world, std::int32_t scene) noexcept;
PVZP_RS_API pvzp_rs_world_clear_lawn_mowers(pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_set_spawn_allowed(pvzp_rs_world* world, std::int32_t zombie_type, std::uint8_t allowed) noexcept;
PVZP_RS_API pvzp_rs_world_set_spawn_entry(pvzp_rs_world* world, std::uint32_t wave, std::uint32_t slot, std::int32_t zombie_type) noexcept;
PVZP_RS_API pvzp_rs_world_pick_spawn_list(pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_start_battle() noexcept;
PVZP_RS_API pvzp_rs_enter_endless(std::int32_t scene) noexcept;
PVZP_RS_API pvzp_rs_enter_native_mode(std::int32_t mode) noexcept;
PVZP_RS_API pvzp_rs_save_current_game() noexcept;
PVZP_RS_API pvzp_rs_back_to_main_menu() noexcept;
PVZP_RS_API pvzp_rs_world_reset(
	pvzp_rs_world* world, std::uint32_t completed_rounds, std::uint32_t seed,
	std::uint32_t initial_sun, std::uint8_t ready_cooldowns) noexcept;
PVZP_RS_API pvzp_rs_set_game_speed(float speed) noexcept;
PVZP_RS_API pvzp_rs_game_speed_multiplier(double* value) noexcept;
PVZP_RS_API pvzp_rs_set_game_speed_multiplier(double value) noexcept;
PVZP_RS_API pvzp_rs_restore_game_speed() noexcept;
PVZP_RS_API pvzp_rs_set_fast_forward(std::uint8_t enabled, std::int32_t performance, std::uint8_t suppress_window) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_fast_forward_active() noexcept;
PVZP_RS_API pvzp_rs_request_seed_chooser_fast_forward(std::uint32_t max_frames) noexcept;
PVZP_RS_API pvzp_rs_set_advanced_pause(
	std::uint8_t enabled, std::uint8_t draw_mask, std::uint32_t rgba,
	std::uint8_t play_sound, std::uint8_t refresh_cursor_preview) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_advanced_pause_active() noexcept;
PVZP_RS_API pvzp_rs_release_advanced_pause_for_step(std::uint8_t play_sound) noexcept;
PVZP_RS_API pvzp_rs_set_random_mode(std::int32_t mode, std::uint32_t value) noexcept;
PVZP_RS_API pvzp_rs_set_wave_spawn_random_seed(std::uint8_t enabled, std::uint32_t seed) noexcept;
PVZP_RS_API pvzp_rs_random_seed(std::int32_t stream, std::uint32_t* out_value) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_random_locked(bool level_stream) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_random_fixed(bool level_stream) noexcept;
PVZP_RS_API pvzp_rs_set_sun_production_mode(std::int32_t mode) noexcept;
PVZP_RS_API pvzp_rs_world_grid_to_pixel_x(const pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, std::int32_t* out_x) noexcept;
PVZP_RS_API pvzp_rs_world_grid_to_pixel_y(const pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, std::int32_t* out_y) noexcept;
PVZP_RS_API pvzp_rs_world_pos_y_based_on_row(const pvzp_rs_world* world, float pos_x, std::int32_t row, float* out_y) noexcept;
PVZP_RS_API pvzp_rs_world_is_pool_square(const pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, std::uint8_t* out_is_pool) noexcept;
PVZP_RS_API pvzp_rs_world_row_can_have_zombies(const pvzp_rs_world* world, std::int32_t row, std::uint8_t* out_allowed) noexcept;
PVZP_RS_API pvzp_rs_world_seed_count(const pvzp_rs_world* world, std::uint32_t* out_count) noexcept;
PVZP_RS_API pvzp_rs_world_seed_at(pvzp_rs_world* world, std::uint32_t index, pvzp_rs_seed** out_seed) noexcept;
PVZP_RS_API pvzp_rs_seed_can_pick_up(pvzp_rs_seed* seed, std::uint8_t* out_can_pick_up) noexcept;
PVZP_RS_API pvzp_rs_seed_was_planted(pvzp_rs_seed* seed) noexcept;
PVZP_RS_API pvzp_rs_selected_card_count(std::uint32_t* out_count) noexcept;
PVZP_RS_API pvzp_rs_selected_card(std::uint32_t index, std::int32_t* out_packet_type, std::int32_t* out_imitater_type) noexcept;
PVZP_RS_API pvzp_rs_select_card(std::int32_t packet_type, std::int32_t imitater_type) noexcept;
PVZP_RS_API pvzp_rs_world_current_plant_cost(const pvzp_rs_world* world, std::int32_t packet_type, std::int32_t imitater_type, std::int32_t* out_cost) noexcept;
PVZP_RS_API pvzp_rs_world_can_take_sun(const pvzp_rs_world* world, std::int32_t amount, std::uint8_t* out_allowed) noexcept;
PVZP_RS_API pvzp_rs_world_take_sun(pvzp_rs_world* world, std::int32_t amount, std::uint8_t* out_taken) noexcept;
PVZP_RS_API pvzp_rs_world_can_plant_at(const pvzp_rs_world* world, std::int32_t packet_type, std::int32_t grid_x, std::int32_t grid_y, std::int32_t* out_reason) noexcept;
PVZP_RS_API pvzp_rs_world_new_plant(pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, std::int32_t packet_type, std::int32_t imitater_type, pvzp_rs_plant** out_plant) noexcept;
PVZP_RS_API pvzp_rs_world_add_plant(pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, std::int32_t packet_type, std::int32_t imitater_type, pvzp_rs_plant** out_plant) noexcept;
PVZP_RS_API pvzp_rs_plant_die(pvzp_rs_plant* plant) noexcept;
PVZP_RS_API pvzp_rs_plant_shovel(pvzp_rs_plant* plant) noexcept;
PVZP_RS_API pvzp_rs_plant_imitater_morph(pvzp_rs_plant* plant, pvzp_rs_plant** out_plant) noexcept;
PVZP_RS_API pvzp_rs_plant_set_sleeping(pvzp_rs_plant* plant, std::uint8_t asleep) noexcept;
PVZP_RS_API pvzp_rs_plant_play_idle(pvzp_rs_plant* plant, float fps) noexcept;
PVZP_RS_API pvzp_rs_plant_update_reanim_color(pvzp_rs_plant* plant) noexcept;
PVZP_RS_API pvzp_rs_plant_fire_cob(pvzp_rs_plant* plant, std::int32_t target_x, std::int32_t target_y) noexcept;
PVZP_RS_EXPORT void pvzp_rs_plant_hit_box(pvzp_rs_plant* plant, pvzp_rs_rect_i32* outRect) noexcept;
PVZP_RS_EXPORT void pvzp_rs_plant_attack_rect(pvzp_rs_plant* plant, std::int32_t weapon, pvzp_rs_rect_i32* outRect) noexcept;
PVZP_RS_EXPORT std::uint32_t pvzp_rs_plant_damage_range_flags(pvzp_rs_plant* plant, std::int32_t weapon) noexcept;
PVZP_RS_API pvzp_rs_plant_reanimation(pvzp_rs_plant* plant, pvzp_rs_reanimation** out_reanimation) noexcept;
PVZP_RS_API pvzp_rs_zombie_pos_y_based_on_row(pvzp_rs_zombie* zombie, std::int32_t row, float* out_y) noexcept;
PVZP_RS_API pvzp_rs_zombie_hit_box(pvzp_rs_zombie* zombie, pvzp_rs_rect_i32* out_rect) noexcept;
PVZP_RS_API pvzp_rs_zombie_effected_by_damage(pvzp_rs_zombie* zombie, std::uint32_t flags, std::uint8_t* out_effected) noexcept;
PVZP_RS_API pvzp_rs_zombie_can_target_plant(pvzp_rs_zombie* zombie, pvzp_rs_plant* plant, std::int32_t attack_type, std::uint8_t* out_can_target) noexcept;
PVZP_RS_EXPORT std::uint8_t pvzp_rs_zombie_is_dead_or_dying(pvzp_rs_zombie* zombie) noexcept;
PVZP_RS_API pvzp_rs_zombie_reanimation(pvzp_rs_zombie* zombie, pvzp_rs_reanimation** out_reanimation) noexcept;
PVZP_RS_API pvzp_rs_world_add_zombie_in_row(pvzp_rs_world* world, std::int32_t zombie_type, std::int32_t row, std::int32_t from_wave, pvzp_rs_zombie** out_zombie) noexcept;
PVZP_RS_API pvzp_rs_world_place_zombie(pvzp_rs_world* world, std::int32_t zombie_type, std::int32_t grid_x, std::int32_t grid_y, pvzp_rs_zombie** out_zombie) noexcept;
PVZP_RS_API pvzp_rs_zombie_die_no_loot(pvzp_rs_zombie* zombie) noexcept;
PVZP_RS_API pvzp_rs_zombie_die_with_loot(pvzp_rs_zombie* zombie) noexcept;
PVZP_RS_EXPORT void pvzp_rs_projectile_hit_box(pvzp_rs_projectile* projectile, pvzp_rs_rect_i32* outRect) noexcept;
PVZP_RS_API pvzp_rs_world_add_ladder(pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, pvzp_rs_grid_item** out_item) noexcept;
PVZP_RS_API pvzp_rs_world_add_crater(pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, pvzp_rs_grid_item** out_item) noexcept;
PVZP_RS_API pvzp_rs_world_add_gravestone(pvzp_rs_world* world, std::int32_t grid_x, std::int32_t grid_y, pvzp_rs_grid_item** out_item) noexcept;
PVZP_RS_API pvzp_rs_grid_item_die(pvzp_rs_grid_item* item) noexcept;
PVZP_RS_API pvzp_rs_item_collect(pvzp_rs_item* item, std::uint8_t play_sound) noexcept;

#define PVZP_RS_DECLARE_BOOL_RULE(name) \
	PVZP_RS_EXPORT std::uint8_t pvzp_rs_##name() noexcept; \
	PVZP_RS_API pvzp_rs_set_##name(std::uint8_t enabled) noexcept

PVZP_RS_DECLARE_BOOL_RULE(seed_recharge_ignored);
PVZP_RS_DECLARE_BOOL_RULE(sun_cost_ignored);
PVZP_RS_DECLARE_BOOL_RULE(fog_revealed);
PVZP_RS_DECLARE_BOOL_RULE(vase_contents_visible);
PVZP_RS_DECLARE_BOOL_RULE(instant_ice_and_ash_effects);
PVZP_RS_EXPORT std::int32_t pvzp_rs_plant_special_countdown_rule() noexcept;
PVZP_RS_API pvzp_rs_set_plant_special_countdown_rule(std::int32_t rule) noexcept;
PVZP_RS_DECLARE_BOOL_RULE(mushrooms_awake);
PVZP_RS_DECLARE_BOOL_RULE(cob_fixed_delay);
PVZP_RS_DECLARE_BOOL_RULE(cob_recharge_shortened);
PVZP_RS_DECLARE_BOOL_RULE(cob_drift_fixed);
PVZP_RS_DECLARE_BOOL_RULE(item_drop_disabled);
PVZP_RS_DECLARE_BOOL_RULE(natural_sun_drop_disabled);
PVZP_RS_DECLARE_BOOL_RULE(jack_explosions_disabled);
PVZP_RS_EXPORT std::int32_t pvzp_rs_jack_explosion_rule() noexcept;
PVZP_RS_API pvzp_rs_set_jack_explosion_rule(std::int32_t rule) noexcept;
PVZP_RS_DECLARE_BOOL_RULE(pepper_explosions_disabled);
PVZP_RS_EXPORT std::int32_t pvzp_rs_pepper_explosion_rule() noexcept;
PVZP_RS_API pvzp_rs_set_pepper_explosion_rule(std::int32_t rule) noexcept;
PVZP_RS_DECLARE_BOOL_RULE(special_events_disabled);
PVZP_RS_DECLARE_BOOL_RULE(zombie_spawn_stopped);
PVZP_RS_DECLARE_BOOL_RULE(zombies_die_at_house);
PVZP_RS_DECLARE_BOOL_RULE(planting_restrictions_ignored);
PVZP_RS_DECLARE_BOOL_RULE(profile_readonly);
PVZP_RS_DECLARE_BOOL_RULE(craters_expire_immediately);
PVZP_RS_DECLARE_BOOL_RULE(chilled_effects_disabled);
PVZP_RS_DECLARE_BOOL_RULE(butter_effects_disabled);
PVZP_RS_DECLARE_BOOL_RULE(imp_throw_disabled);
PVZP_RS_DECLARE_BOOL_RULE(zombie_walking_stopped);
PVZP_RS_DECLARE_BOOL_RULE(ice_trails_disabled);
PVZP_RS_DECLARE_BOOL_RULE(pause_dialog_disabled);
PVZP_RS_DECLARE_BOOL_RULE(hidden_pages_visible);
PVZP_RS_EXPORT std::int32_t pvzp_rs_background_run_rule() noexcept;
PVZP_RS_API pvzp_rs_set_background_run_rule(std::int32_t rule) noexcept;
std::uint8_t pvzp_rs_window_left_aligned() noexcept;
PVZP_RS_API pvzp_rs_set_window_left_aligned(std::uint8_t enabled) noexcept;
std::uint8_t pvzp_rs_window_expanded() noexcept;
PVZP_RS_API pvzp_rs_set_window_expanded(std::uint8_t enabled) noexcept;
double pvzp_rs_window_scale() noexcept;
PVZP_RS_API pvzp_rs_set_window_scale(double scale) noexcept;
std::int32_t pvzp_rs_window_view_width() noexcept;
std::uint8_t pvzp_rs_window_resizable() noexcept;
PVZP_RS_API pvzp_rs_set_window_resizable(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_clear_ice_trails() noexcept;
PVZP_RS_API pvzp_rs_set_ice_trail_countdown(std::uint8_t row, std::int32_t value) noexcept;
PVZP_RS_DECLARE_BOOL_RULE(normal_auto_collect_enabled);

#undef PVZP_RS_DECLARE_BOOL_RULE

PVZP_RS_API pvzp_rs_easy_planting_cheat(std::uint8_t* out_enabled) noexcept;
PVZP_RS_API pvzp_rs_set_easy_planting_cheat(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_kernel_pult_projectile_rule(std::int32_t* out_rule) noexcept;
PVZP_RS_API pvzp_rs_set_kernel_pult_projectile_rule(std::int32_t rule) noexcept;
PVZP_RS_API pvzp_rs_zombie_damage_rule(std::int32_t* out_rule) noexcept;
PVZP_RS_API pvzp_rs_set_zombie_damage_rule(std::int32_t rule) noexcept;
PVZP_RS_API pvzp_rs_plant_damage_rule(std::int32_t* out_rule) noexcept;
PVZP_RS_API pvzp_rs_set_plant_damage_rule(std::int32_t rule) noexcept;
PVZP_RS_API pvzp_rs_maid_cheat(std::int32_t* out_cheat) noexcept;
PVZP_RS_API pvzp_rs_set_maid_cheat(std::int32_t cheat) noexcept;
PVZP_RS_EXPORT pvzp_rs_plant_pool* pvzp_rs_world_plant_pool(pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT pvzp_rs_zombie_pool* pvzp_rs_world_zombie_pool(pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT pvzp_rs_projectile_pool* pvzp_rs_world_projectile_pool(pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT pvzp_rs_grid_item_pool* pvzp_rs_world_grid_item_pool(pvzp_rs_world* world) noexcept;
PVZP_RS_EXPORT pvzp_rs_item_pool* pvzp_rs_world_item_pool(pvzp_rs_world* world) noexcept;
pvzp_rs_mower_pool* pvzp_rs_world_mower_pool(pvzp_rs_world* world) noexcept;
std::uint32_t pvzp_rs_mower_id(const pvzp_rs_mower* mower) noexcept;
PVZP_RS_API pvzp_rs_mower_start(pvzp_rs_mower* mower) noexcept;
PVZP_RS_API pvzp_rs_mower_die(pvzp_rs_mower* mower) noexcept;
PVZP_RS_API pvzp_rs_world_initialize_mowers_ready(pvzp_rs_world* world) noexcept;

#define PVZP_RS_DECLARE_POOL_API(prefix, pool_type, object_type) \
	PVZP_RS_EXPORT std::uint32_t pvzp_rs_##prefix##_pool_max_used_count(const pool_type* pool) noexcept; \
	PVZP_RS_EXPORT std::uint32_t pvzp_rs_##prefix##_pool_active_count(const pool_type* pool) noexcept; \
	PVZP_RS_EXPORT std::uint32_t pvzp_rs_##prefix##_pool_capacity(const pool_type* pool) noexcept; \
	PVZP_RS_EXPORT std::uint32_t pvzp_rs_##prefix##_pool_free_list_head(const pool_type* pool) noexcept; \
	PVZP_RS_EXPORT std::uint32_t pvzp_rs_##prefix##_pool_next_id_key(const pool_type* pool) noexcept; \
	PVZP_RS_API pvzp_rs_##prefix##_pool_slot(const pool_type* pool, std::uint32_t index, std::uint8_t* out_active, std::uint32_t* out_id, std::uint32_t* out_next_free) noexcept; \
	PVZP_RS_API pvzp_rs_##prefix##_pool_get(pool_type* pool, std::int32_t index, object_type** out_object) noexcept; \
	PVZP_RS_EXPORT object_type* pvzp_rs_##prefix##_pool_try_to_get(pool_type* pool, std::uint32_t id) noexcept; \
	PVZP_RS_API pvzp_rs_##prefix##_pool_get_id(const pool_type* pool, const object_type* object, std::uint32_t* out_id) noexcept

PVZP_RS_DECLARE_POOL_API(plant, pvzp_rs_plant_pool, pvzp_rs_plant);
PVZP_RS_DECLARE_POOL_API(zombie, pvzp_rs_zombie_pool, pvzp_rs_zombie);
PVZP_RS_DECLARE_POOL_API(projectile, pvzp_rs_projectile_pool, pvzp_rs_projectile);
PVZP_RS_DECLARE_POOL_API(grid_item, pvzp_rs_grid_item_pool, pvzp_rs_grid_item);
PVZP_RS_DECLARE_POOL_API(item, pvzp_rs_item_pool, pvzp_rs_item);
PVZP_RS_DECLARE_POOL_API(mower, pvzp_rs_mower_pool, pvzp_rs_mower);

#undef PVZP_RS_DECLARE_POOL_API

PVZP_RS_EXPORT void rsvz_pvzp_log(const char* message, std::size_t length);
PVZP_RS_EXPORT void rsvz_pvzp_restore_all();


std::uint8_t pvzp_rs_seed_bank_topmost() noexcept;
PVZP_RS_API pvzp_rs_set_seed_bank_topmost(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_dance_mode_enabled(std::uint8_t* enabled) noexcept;
PVZP_RS_API pvzp_rs_restore_dance_mode(std::uint8_t enabled) noexcept;
PVZP_RS_API pvzp_rs_seed_bank_visible(std::uint8_t* visible) noexcept;
PVZP_RS_API pvzp_rs_set_seed_bank_visible(std::uint8_t visible) noexcept;
PVZP_RS_API pvzp_rs_world_seed_storage_at(pvzp_rs_world* world, std::uint32_t index, pvzp_rs_seed** out_seed) noexcept;
PVZP_RS_API pvzp_rs_world_set_seed_count(pvzp_rs_world* world, std::uint8_t count) noexcept;
PVZP_RS_API pvzp_rs_world_grid_square_kind(pvzp_rs_world* world, std::int32_t row, std::int32_t col, std::int32_t* kind) noexcept;
PVZP_RS_API pvzp_rs_world_grid_row_kind(pvzp_rs_world* world, std::int32_t row, std::int32_t* kind) noexcept;
PVZP_RS_API pvzp_rs_world_spawn_challenge_grave(pvzp_rs_world* world, std::int32_t row, std::int32_t col) noexcept;
PVZP_RS_API pvzp_rs_world_place_rake(pvzp_rs_world* world, std::int32_t row, std::int32_t col, std::uint8_t* placed) noexcept;
PVZP_RS_API pvzp_rs_world_process_delete_queue(pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_native_game_mode(std::int32_t* mode) noexcept;
PVZP_RS_API pvzp_rs_frame_duration_ms(std::int32_t* out_value) noexcept;
PVZP_RS_API pvzp_rs_plant_setup_izombie(pvzp_rs_plant* plant) noexcept;
PVZP_RS_API pvzp_rs_world_validate_scene_layout(pvzp_rs_world* world, std::int32_t scene) noexcept;
PVZP_RS_API pvzp_rs_world_set_scene_layout(pvzp_rs_world* world, std::int32_t scene) noexcept;
PVZP_RS_API pvzp_rs_world_spawn_huge_wave(pvzp_rs_world* world, std::uint32_t wave, std::uint8_t* huge) noexcept;
PVZP_RS_API pvzp_rs_world_refresh_spawn_preview(pvzp_rs_world* world) noexcept;
PVZP_RS_API pvzp_rs_world_board_spawn_seed(pvzp_rs_world* world,std::uint32_t* seed) noexcept;
PVZP_RS_API pvzp_rs_world_generate_current_spawn(pvzp_rs_world* world,std::uint32_t seed) noexcept;

std::uint8_t pvzp_rs_seed_recharge_shading_hidden() noexcept;
PVZP_RS_API pvzp_rs_set_seed_recharge_shading_hidden(std::uint8_t enabled) noexcept;

std::uint8_t pvzp_rs_upgrade_planting_unrestricted() noexcept;
PVZP_RS_API pvzp_rs_set_upgrade_planting_unrestricted(std::uint8_t enabled) noexcept;

// Retained options of one native pause request, matching the setter above.
void pvzp_rs_advanced_pause_options(std::uint8_t* draw_mask, std::uint32_t* rgba, std::uint8_t* cursor) noexcept;

PVZP_RS_API pvzp_rs_world_plant_slot_id(pvzp_rs_world* world, std::uint32_t slot, std::uint32_t* id) noexcept;
PVZP_RS_API pvzp_rs_world_relocate_plant_slot(pvzp_rs_world* world, std::uint32_t source, std::uint32_t target, std::uint32_t expected_target, std::uint32_t* moved, std::uint32_t* swapped, std::uint8_t commit) noexcept;


PVZP_RS_API pvzp_rs_chooser_card_available(std::int32_t kind, std::uint8_t* available) noexcept;
PVZP_RS_API pvzp_rs_chooser_card_locked(std::uint32_t index, std::uint8_t* locked) noexcept;
PVZP_RS_API pvzp_rs_deselect_card(std::uint32_t index) noexcept;
PVZP_RS_API pvzp_rs_stop_seed_chooser_fast_forward() noexcept;

#undef PVZP_RS_API

}

#undef PVZP_RS_EXPORT
extern "C" {
std::uint8_t pvzp_rs_replay_mouse_x(std::int32_t* value) noexcept;
std::uint8_t pvzp_rs_replay_mouse_y(std::int32_t* value) noexcept;
std::uint8_t pvzp_rs_replay_mouse_buttons(std::int32_t* value) noexcept;
std::uint8_t pvzp_rs_replay_cursor_kind(std::int32_t* value) noexcept;
std::uint8_t pvzp_rs_replay_refresh() noexcept;
}
extern "C" std::uint8_t pvzp_rs_clear_projectiles() noexcept; extern "C" std::uint8_t pvzp_rs_clear_items() noexcept;

extern "C" std::int32_t pvzp_rs_replay_emitter_count(std::int32_t kind) noexcept;
extern "C" std::int32_t pvzp_rs_replay_track_count(std::int32_t kind) noexcept;
extern "C" std::int32_t pvzp_rs_replay_frame_count(std::int32_t kind) noexcept;

#include "widget/TouchInput.h"
extern "C" {
std::uint8_t pvzp_rs_input_begin(std::uint8_t playback) noexcept;
std::uint8_t pvzp_rs_input_drain(Sexy::PointerInput* output, std::size_t capacity, std::size_t* count) noexcept;
std::uint8_t pvzp_rs_input_apply(const Sexy::PointerInput* input) noexcept;
void pvzp_rs_input_end() noexcept;
void pvzp_rs_input_hand_back() noexcept;
std::uint64_t pvzp_rs_input_sequence() noexcept;
}

extern "C" std::int32_t pvzp_rs_input_terminal_clock() noexcept;

extern "C" std::uint8_t pvzp_rs_checkpoint_ready() noexcept;

#if defined(__ANDROID__)
extern "C" {
void pvzp_rs_clear_screen_buttons(std::uint32_t source) noexcept;
bool pvzp_rs_set_screen_button(std::uint32_t source, std::uint32_t id, bool right, std::uint32_t first,
    std::uint32_t span, const std::uint32_t* pixels, std::int32_t width, std::int32_t height) noexcept;
std::uint32_t pvzp_rs_take_screen_button(std::uint32_t source) noexcept;
bool pvzp_rs_screen_buttons_available(std::uint32_t source) noexcept;
}
#endif

extern "C" std::int32_t pvzp_rs_diagnostic_dialog(const char* text) noexcept;
extern "C" void pvzp_rs_diagnostic_exit() noexcept;
