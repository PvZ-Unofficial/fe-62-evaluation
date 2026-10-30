// Included in bridge.cpp to share its existing checked world and status helpers.
extern "C" pvzp_rs_status pvzp_rs_cursor_position(const pvzp_rs_world* world, std::int32_t* x, std::int32_t* y) noexcept {
    if (!world || !x || !y || !Native(world)->mApp->mWidgetManager) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *x=Native(world)->mApp->mWidgetManager->mLastMouseX;
    *y=Native(world)->mApp->mWidgetManager->mLastMouseY; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_cursor_object_position(const pvzp_rs_world* world, std::int32_t* x, std::int32_t* y) noexcept {
    if (!world || !x || !y || !Native(world)->mCursorObject) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *x=Native(world)->mCursorObject->mX; *y=Native(world)->mCursorObject->mY; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_cursor_grid_row(const pvzp_rs_world* world, std::int32_t* value) noexcept {
    if (!world || !value || !Native(world)->mCursorPreview) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *value=Native(world)->mCursorPreview->mGridY; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_cursor_seed_slot(const pvzp_rs_world* world, std::int32_t* value) noexcept {
    if (!world || !value || !Native(world)->mCursorObject || !Native(world)->mSeedBank) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    const auto* cursor=Native(world)->mCursorObject.get();
    *value=-1;
    if (cursor->mCursorType!=CursorType::CURSOR_TYPE_PLANT_FROM_BANK) return Ok();
    if (cursor->mSeedBankIndex<0 || cursor->mSeedBankIndex>=Native(world)->mSeedBank->mNumPackets) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *value=cursor->mSeedBankIndex; return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_select_cob(pvzp_rs_world* world, std::uint32_t id) noexcept {
    if (!world || !Native(world)->mCursorObject) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    auto* board=Native(world); auto* plant=board->mPlants.DataArrayTryToGet(id);
    if (!plant || plant->mDead || plant->mSeedType!=SeedType::SEED_COBCANNON || static_cast<int>(plant->mState)!=37) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    board->mCursorObject->mCobCannonPlantID=static_cast<PlantID>(id);
    board->mCursorObject->mCursorType=CursorType::CURSOR_TYPE_COBCANNON_TARGET; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_new_plant_native_grid(pvzp_rs_world* world, std::int32_t x, std::int32_t y, std::int32_t kind, std::int32_t imitater, pvzp_rs_plant** out) noexcept {
    return Guard([&] {
        if (!world || !out) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (x<0 || x>=9 || y<0 || y>=6 || kind<0 || kind>48 || imitater < -1 || imitater>47) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        auto* board=Native(world);
        if (board->mPlants.mSize>=board->mPlants.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        *out=board->NewPlant(x,y,static_cast<SeedType>(kind),static_cast<SeedType>(imitater)); return Ok();
    });
}

namespace PvzpNative { PVZP_API bool HouseEntryPauseEnabled(); PVZP_API void SetHouseEntryPause(bool); PVZP_API bool TakeHouseEntry(); }
extern "C" pvzp_rs_status pvzp_rs_board_view(const pvzp_rs_world* world,std::int32_t* x,std::int32_t* y) noexcept {
    if(!world||!x||!y)return Fail(PVZP_RS_STATUS_NULL_POINTER);*x=Native(world)->mX;*y=Native(world)->mY;return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_board_view(pvzp_rs_world* world,std::int32_t x,std::int32_t y) noexcept {
    if(!world)return Fail(PVZP_RS_STATUS_NULL_POINTER);Native(world)->mX=x;Native(world)->mY=y;return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_animation_skip(std::uint8_t* out) noexcept {if(!gLawnApp||!out)return Fail(PVZP_RS_STATUS_NULL_POINTER);*out=gLawnApp->mCheatKeys;return Ok();}
extern "C" pvzp_rs_status pvzp_rs_set_animation_skip(std::uint8_t value) noexcept {if(!gLawnApp)return Fail(PVZP_RS_STATUS_NULL_POINTER);gLawnApp->mCheatKeys=value!=0;return Ok();}
extern "C" pvzp_rs_status pvzp_rs_house_pause(std::uint8_t* out) noexcept {if(!out)return Fail(PVZP_RS_STATUS_NULL_POINTER);*out=PvzpNative::HouseEntryPauseEnabled();return Ok();}
extern "C" pvzp_rs_status pvzp_rs_set_house_pause(std::uint8_t value) noexcept {PvzpNative::SetHouseEntryPause(value!=0);return Ok();}
extern "C" pvzp_rs_status pvzp_rs_take_house(std::uint8_t* out) noexcept {if(!out)return Fail(PVZP_RS_STATUS_NULL_POINTER);*out=PvzpNative::TakeHouseEntry();return Ok();}
namespace PvzpNative {PVZP_API void EnsureMusicTune(std::int32_t tune);}
extern "C" pvzp_rs_status pvzp_rs_scene_music(std::int32_t tune) noexcept {
    if(!gLawnApp||!gLawnApp->mMusic)return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if(tune<1||tune>13)return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    PvzpNative::EnsureMusicTune(tune);return Ok();
}

namespace PvzpNative {PVZP_API void ClearFastMode();}
extern "C" pvzp_rs_status pvzp_rs_clear_fast_mode() noexcept {PvzpNative::ClearFastMode();return Ok();}

namespace PvzpNative {PVZP_API void SetBoardClickCapture(bool);PVZP_API int TakeBoardClick(int*,int*,int*);}
extern "C" void pvzp_rs_set_board_click_native_priority(bool enabled) noexcept { PvzpNative::SetBoardClickNativePriority(enabled); }
extern "C" bool pvzp_rs_board_click_capture_active() noexcept { return PvzpNative::BoardClickCaptureActive(); }
extern "C" void pvzp_rs_set_lawn_preview(int kind, bool imitate) noexcept { PvzpNative::SetLawnPreview(kind, imitate); }
extern "C" void pvzp_rs_set_board_click_capture(bool enabled) noexcept {PvzpNative::SetBoardClickCapture(enabled);}
extern "C" int pvzp_rs_take_board_click(int* x,int* y,int* button) noexcept {return PvzpNative::TakeBoardClick(x,y,button);}

#if defined(__ANDROID__)
#include "PvzpLib/ScreenButtons.h"
extern "C" void pvzp_rs_clear_screen_buttons(std::uint32_t source) noexcept { PvzpNative::ClearScreenButtons(source); }
extern "C" bool pvzp_rs_set_screen_button(std::uint32_t source,std::uint32_t id,bool right,std::uint32_t first,
    std::uint32_t span,const std::uint32_t* pixels,std::int32_t width,std::int32_t height) noexcept {
    try { return PvzpNative::SetScreenButton(source,id,right,first,span,pixels,width,height); } catch (...) { return false; }
}
extern "C" std::uint32_t pvzp_rs_take_screen_button(std::uint32_t source) noexcept { return PvzpNative::TakeScreenButton(source); }
extern "C" bool pvzp_rs_screen_buttons_available(std::uint32_t source) noexcept { return PvzpNative::ScreenButtonsAvailable(source); }
#endif
