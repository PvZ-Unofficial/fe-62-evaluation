#include "bridge.h"
#include "PvzpLib/Paint.h"
#include "PvzpLib/NativeControls.h"
#include "PvzpLib/EffectSystem.h"
#include "PvzpLib/PluginLayout.h"


#include <cstdlib>
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstring>
#include <exception>
#include <climits>
#include <new>
#include <string_view>
#include <type_traits>

#include "Lawn/Board.h"
#include "Lawn/Challenge.h"
#include "Lawn/MessageWidget.h"
#include "Lawn/CursorObject.h"
#include "Lawn/Cutscene.h"
#include "Lawn/LawnMower.h"
#include "Lawn/System/Music.h"
#include "Lawn/System/PlayerInfo.h"
#include "Lawn/Widget/GameButton.h"
#include "Lawn/Widget/GameSelector.h"
#include "Lawn/Widget/SeedChooserScreen.h"
#include "Lawn/Widget/TitleScreen.h"
#include "LawnApp.h"
#include "PvzpLib/Replay.h"
#include "PvzpLib/PvzpDebug.h"
#include "PvzpLib/PvzpParticle.h"
#include "SexyAppFramework/Common.h"
#include "SexyAppFramework/graphics/Graphics.h"
#include "SexyAppFramework/misc/MTRand.h"
#include "SexyAppFramework/widget/WidgetManager.h"
#include "SexyAppFramework/sound/SoundManager.h"
#include "SexyAppFramework/sound/SoundInstance.h"


using namespace PvzpNative;

extern "C" void pvzp_rs_replay_gargantuar_damage(pvzp_rs_zombie* zombie) noexcept {
    PvzpReplay::GargantuarDamage(zombie);
    PvzpReplay::FootballDamage(zombie);
}

extern "C" void pvzp_rs_replay_plant_damage(pvzp_rs_plant* plant) noexcept {
    PvzpReplay::PlantDamage(plant);
}
extern "C" void pvzp_rs_replay_digger_facing(pvzp_rs_reanimation* animation, float fractionalX, bool right) noexcept {
    if (!animation) return;
    // Zombie::UpdateReanim's mirror and 90px compensation, without advancing animation.
    animation->mOverlayMatrix.m00 = right ? -1.0f : 1.0f;
    animation->mOverlayMatrix.m02 = fractionalX + 15.0f + (right ? 90.0f : 0.0f);
}

extern "C" void pvzp_rs_replay_clock(pvzp_rs_world* world, std::uint32_t clock, std::int32_t wave) noexcept {
    auto* board=reinterpret_cast<Board*>(world); if (!board) return;
    board->mMainCounter=clock; board->mCurrentWave=wave;
}
extern "C" void pvzp_rs_replay_ice(pvzp_rs_world* world, std::uint32_t row, std::int32_t x, std::int32_t ticks) noexcept {
    auto* board=reinterpret_cast<Board*>(world); if (!board || row>=6) return;
    board->mIceMinX[row]=x; board->mIceTimer[row]=ticks;
}
extern "C" pvzp_rs_projectile* pvzp_rs_replay_projectile(pvzp_rs_world* world,int kind,int row,int x,int y) noexcept {
    auto* board=reinterpret_cast<Board*>(world); if (!board || kind<0 || kind>12 || row<0 || row>5) return nullptr;
    try { return board->AddProjectile(x,y,400000+row*10000,row,static_cast<ProjectileType>(kind)); } catch (...) { return nullptr; }
}

extern "C" std::uint8_t pvzp_rs_window_left_aligned() noexcept { return WindowLeftAligned(); }
extern "C" pvzp_rs_status pvzp_rs_set_window_left_aligned(std::uint8_t enabled) noexcept {
    if (!gLawnApp || !gLawnApp->mWindow) return {PVZP_RS_STATUS_NULL_POINTER, 0};
    try { SetWindowLeftAligned(enabled != 0); return {PVZP_RS_STATUS_OK, 0}; }
    catch (...) { return {PVZP_RS_STATUS_STD_EXCEPTION, 0}; }
}
extern "C" std::uint8_t pvzp_rs_window_expanded() noexcept { return WindowExpanded(); }
extern "C" double pvzp_rs_window_scale() noexcept { return WindowScale(); }
extern "C" std::int32_t pvzp_rs_window_view_width() noexcept { return WindowViewWidth(); }
extern "C" pvzp_rs_status pvzp_rs_set_window_expanded(std::uint8_t enabled) noexcept {
    if (!gLawnApp || !gLawnApp->mWindow) return {PVZP_RS_STATUS_NULL_POINTER, 0};
    try { SetWindowExpanded(enabled != 0); return {PVZP_RS_STATUS_OK, 0}; }
    catch (...) { return {PVZP_RS_STATUS_STD_EXCEPTION, 0}; }
}
extern "C" pvzp_rs_status pvzp_rs_set_window_scale(double scale) noexcept {
    if (!gLawnApp || !gLawnApp->mWindow) return {PVZP_RS_STATUS_NULL_POINTER, 0};
    try { SetWindowScale(scale); return {PVZP_RS_STATUS_OK, 0}; }
    catch (const std::invalid_argument&) { return {PVZP_RS_STATUS_INVALID_ARGUMENT, 0}; }
    catch (...) { return {PVZP_RS_STATUS_STD_EXCEPTION, 0}; }
}

extern "C" std::uint8_t pvzp_rs_window_resizable() noexcept
{
    return WindowResizable();
}

extern "C" pvzp_rs_status pvzp_rs_set_window_resizable(std::uint8_t enabled) noexcept
{
    if (!gLawnApp || !gLawnApp->mWindow) return {PVZP_RS_STATUS_NULL_POINTER, 0};
    SetWindowResizable(enabled != 0);
    return {PVZP_RS_STATUS_OK, 0};
}

extern "C" pvzp_rs_status pvzp_rs_native_game_mode(std::int32_t* mode) noexcept {
    if(!gLawnApp||!mode)return {PVZP_RS_STATUS_NULL_POINTER,0};
    *mode=static_cast<std::int32_t>(gLawnApp->mGameMode);return {PVZP_RS_STATUS_OK,0};
}


extern "C" std::uint8_t pvzp_rs_temporary_unlock_enabled() noexcept
{
    return gModifiers.temporaryUnlock;
}



static_assert(PVZP_RS_SPAWN_WAVE_CAPACITY == MAX_ZOMBIE_WAVES);

extern "C" std::uint8_t pvzp_rs_check_raw_layout(const pvzp_rs_layout_entry* entries, std::uint32_t count) noexcept
{
    if (!entries || !count) return 0;
    for (std::uint32_t i = 0; i < count; ++i) {
        bool found = false;
        for (const auto& native : PvzpPlugin::Layout) {
            if (native.name != entries[i].name) continue;
            if (native.value != entries[i].value) return 0;
            found = true;
            break;
        }
        if (!found) return 0;
    }
    return 1;
}

extern "C" std::uint8_t pvzp_rs_validate_abi() noexcept
{
    return PvzpPlugin::ValidateLayout(PvzpPlugin::Layout, sizeof(PvzpPlugin::Layout) / sizeof(PvzpPlugin::Layout[0]));
}
extern "C" std::uint8_t pvzp_rs_register_update(PvzpPlugin::UpdateCallback update, PvzpPlugin::BoardDestroyingCallback destroy) noexcept
{
    return PvzpPlugin::SetUpdateCallback(update) && PvzpPlugin::SetBoardDestroyingCallback(destroy);
}
extern "C" std::uint8_t pvzp_rs_register_battle(const pvzp_rs_battle_callbacks* callbacks, std::uint32_t interest) noexcept
{
    return PvzpPlugin::SetBattleCallbacks(callbacks, interest);
}
extern "C" void pvzp_rs_clear_battle() noexcept { PvzpPlugin::ClearBattleCallbacks(); }
extern "C" void pvzp_rs_request_stop() noexcept { PvzpPlugin::RequestStop(); }

namespace {
	constexpr pvzp_rs_status Ok()
	{
		return {PVZP_RS_STATUS_OK, 0};
	}

	constexpr pvzp_rs_status Fail(pvzp_rs_status_code code, std::int32_t detail = 0)
	{
		return {code, detail};
	}

	template <typename F>
	pvzp_rs_status Guard(F&& operation) noexcept
	{
		try
		{
			return operation();
		}
		catch (const std::bad_alloc&)
		{
			return Fail(PVZP_RS_STATUS_BAD_ALLOC);
		}
		catch (const std::exception&)
		{
			return Fail(PVZP_RS_STATUS_STD_EXCEPTION);
		}
		catch (...)
		{
			return Fail(PVZP_RS_STATUS_UNKNOWN_EXCEPTION);
		}
	}
	Board* Native(pvzp_rs_world* world)
	{
		return reinterpret_cast<Board*>(world);
	}

	const Board* Native(const pvzp_rs_world* world)
	{
		return reinterpret_cast<const Board*>(world);
	}

	// Reset keeps stage zero while constructing the chooser. Spawn generation
	// must nevertheless use the requested stage for points, weights and seed.
	struct SpawnStageScope
	{
		Challenge* challenge;
		int previous;
		explicit SpawnStageScope(Board* board)
			: challenge(board->mChallenge.get()), previous(challenge->mSurvivalStage)
		{
			if (gPendingSurvivalStage >= 0)
				challenge->mSurvivalStage = gPendingSurvivalStage;
		}
		~SpawnStageScope() { challenge->mSurvivalStage = previous; }
	};

	void ClearLawnMowers(Board* board)
	{
		const int bonus = board->mBonusLawnMowersRemaining;
		board->mBonusLawnMowersRemaining = 0;
		for (LawnMower* mower : board->mLawnMowers)
			if (!mower->mDead)
				mower->Die();
		board->mBonusLawnMowersRemaining = bonus;
		for (LawnMower* mower : board->mLawnMowers)
			if (mower->mDead)
				board->mLawnMowers.DataArrayFree(mower);
	}

	SeedChooserScreen* CurrentSeedChooser()
	{
		if (!gLawnApp || gLawnApp->mGameScene != GameScenes::SCENE_LEVEL_INTRO)
			return nullptr;
		return gLawnApp->mSeedChooserScreen;
	}

	template <typename T, typename Pool>
	DataArray<T>* NativePool(Pool* pool)
	{
		return reinterpret_cast<DataArray<T>*>(pool);
	}

	template <typename T, typename Pool>
	const DataArray<T>* NativePool(const Pool* pool)
	{
		return reinterpret_cast<const DataArray<T>*>(pool);
	}

	template <typename T, typename Pool>
	pvzp_rs_status PoolSlot(
		const Pool* opaque,
		std::uint32_t index,
		std::uint8_t* outActive,
		std::uint32_t* outId,
		std::uint32_t* outNextFree)
	{
		if (!opaque || !outActive || !outId || !outNextFree)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* pool = const_cast<DataArray<T>*>(NativePool<T>(opaque));
		if (index >= pool->mMaxSize)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(index));
		const std::uint32_t id = pool->DataArrayGetIDAt(index);
		const bool active = (id & DATA_ARRAY_KEY_MASK) != 0;
		*outActive = static_cast<std::uint8_t>(active);
		*outId = active ? id : 0;
		*outNextFree = active ? 0 : id;
		return Ok();
	}

	template <typename T, typename Pool>
	pvzp_rs_status PoolGet(Pool* opaque, std::int32_t index, T** outObject)
	{
		if (!opaque || !outObject)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* pool = NativePool<T>(opaque);
		if (index < 0 || static_cast<std::uint32_t>(index) >= pool->mMaxUsedCount)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, index);
		const std::uint32_t id = pool->DataArrayGetIDAt(static_cast<std::uint32_t>(index));
		if (!(id & DATA_ARRAY_KEY_MASK))
		{
			*outObject = nullptr;
			return Ok();
		}
		*outObject = &pool->DataArrayGetItemAt(static_cast<std::uint32_t>(index));
		return Ok();
	}


	constexpr bool PoolContainsAddress(std::uintptr_t address, std::uintptr_t begin,
		std::uintptr_t stride, std::uint32_t count)
	{
		return stride != 0 && address >= begin && (address - begin) % stride == 0 &&
			(address - begin) / stride < count;
	}

	static_assert(PoolContainsAddress(100, 100, 16, 2));
	static_assert(PoolContainsAddress(116, 100, 16, 2));
	static_assert(!PoolContainsAddress(99, 100, 16, 2));
	static_assert(!PoolContainsAddress(101, 100, 16, 2));
	static_assert(!PoolContainsAddress(132, 100, 16, 2));
	static_assert(!PoolContainsAddress(100, 100, 16, 0));
	static_assert(!PoolContainsAddress(100, 100, 0, 2));

	template <typename T, typename Pool>
	pvzp_rs_status PoolGetId(const Pool* opaque, const T* object, std::uint32_t* outId)
	{
		if (!opaque || !object || !outId)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* pool = const_cast<DataArray<T>*>(NativePool<T>(opaque));
		const auto count = std::min(pool->mMaxUsedCount, pool->mMaxSize);
		if (count == 0)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		const auto begin = reinterpret_cast<std::uintptr_t>(&pool->DataArrayGetItemAt(0));
		const auto address = reinterpret_cast<std::uintptr_t>(object);
		const auto stride = pool->mMaxSize > 1
			? reinterpret_cast<std::uintptr_t>(&pool->DataArrayGetItemAt(1)) - begin
			: sizeof(T);
		// Check membership before indexing IDs; DataArrayGetID assumes this relation already holds.
		if (!PoolContainsAddress(address, begin, stride, count))
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		const auto index = static_cast<std::uint32_t>((address - begin) / stride);
		const auto id = pool->DataArrayGetIDAt(index);
		if ((id & DATA_ARRAY_KEY_MASK) == 0)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		*outId = id;
		return Ok();
	}

	void CopyRect(const Rect& source, pvzp_rs_rect_i32* destination)
	{
		destination->x = source.mX;
		destination->y = source.mY;
		destination->width = source.mWidth;
		destination->height = source.mHeight;
	}

	bool IsPacketType(std::int32_t value)
	{
		return value >= static_cast<std::int32_t>(SeedType::SEED_PEASHOOTER)
			&& value <= static_cast<std::int32_t>(SeedType::SEED_IMITATER);
	}

	bool IsImitaterTarget(std::int32_t value)
	{
		return value == static_cast<std::int32_t>(SeedType::SEED_NONE)
			|| (value >= static_cast<std::int32_t>(SeedType::SEED_PEASHOOTER)
				&& value < static_cast<std::int32_t>(SeedType::SEED_IMITATER));
	}

	bool IsZombieType(std::int32_t value)
	{
		return value >= static_cast<std::int32_t>(ZombieType::ZOMBIE_NORMAL)
			&& value < static_cast<std::int32_t>(ZombieType::NUM_ZOMBIE_TYPES);
	}

	std::uint32_t MixDancerClock(std::uint32_t seed)
	{
		std::uint32_t value = seed ^ 0x9e3779b9U;
		value ^= value >> 16;
		value *= 0x7feb352dU;
		value ^= value >> 15;
		value *= 0x846ca68bU;
		return (value ^ (value >> 16)) % 10000U;
	}

	GameMode EndlessMode(std::int32_t scene)
	{
		switch (scene)
		{
		case 0: return GameMode::GAMEMODE_SURVIVAL_ENDLESS_STAGE_1;
		case 1: return GameMode::GAMEMODE_SURVIVAL_ENDLESS_STAGE_2;
		case 2: return GameMode::GAMEMODE_SURVIVAL_ENDLESS_STAGE_3;
		case 3: return GameMode::GAMEMODE_SURVIVAL_ENDLESS_STAGE_4;
		case 4: return GameMode::GAMEMODE_SURVIVAL_ENDLESS_STAGE_5;
		default: return GameMode::NUM_GAME_MODES;
		}
	}

}

extern "C" void rsvz_pvzp_log(const char* message, std::size_t length)
{
	PvzpTraceAndLogLn("%.*s", static_cast<int>(length), message);
}

extern "C" pvzp_rs_status pvzp_rs_set_common_dance(std::int32_t dance) noexcept
{
    if (dance < 0 || dance > 2) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, dance);
    gModifiers.commonZombieDance = static_cast<CommonDance>(dance);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_dance_mode(std::uint8_t enabled) noexcept
{
    if (!gLawnApp || !gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    gLawnApp->mBoard->SetDanceMode(enabled != 0);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_dance_mode_enabled(std::uint8_t* enabled) noexcept {
    if (!gLawnApp || !enabled) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *enabled = gLawnApp->mDanceMode;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_restore_dance_mode(std::uint8_t enabled) noexcept {
    if (!gLawnApp) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (gLawnApp->mBoard) gLawnApp->mBoard->SetDanceMode(enabled != 0);
    else gLawnApp->mDanceMode = enabled != 0;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_seed_bank_visible(std::uint8_t* visible) noexcept {
    if (!gLawnApp || !gLawnApp->mBoard || !gLawnApp->mBoard->mSeedBank || !visible) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *visible = gLawnApp->mBoard->mSeedBank->mVisible;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_seed_bank_visible(std::uint8_t visible) noexcept {
    if (!gLawnApp || !gLawnApp->mBoard || !gLawnApp->mBoard->mSeedBank) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    gLawnApp->mBoard->mSeedBank->mVisible = visible != 0;
    gLawnApp->mBoard->MarkDirty();
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_seed_storage_at(pvzp_rs_world* world, std::uint32_t index, pvzp_rs_seed** outSeed) noexcept {
    if (!world || !outSeed || !Native(world)->mSeedBank) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (index >= SEEDBANK_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *outSeed = &Native(world)->mSeedBank->mSeedPackets[index];
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_set_seed_count(pvzp_rs_world* world, std::uint8_t count) noexcept {
    return Guard([&] {
        if (!world || !Native(world)->mSeedBank) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (!count || count > SEEDBANK_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        Native(world)->mSeedBank->SetPacketCount(count);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_grid_square_kind(pvzp_rs_world* world, std::int32_t row, std::int32_t col, std::int32_t* kind) noexcept {
    if (!world || !kind) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (row<0 || row>=MAX_GRID_SIZE_Y || col<0 || col>=MAX_GRID_SIZE_X) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *kind=static_cast<std::int32_t>(Native(world)->mGridSquareType[col][row]);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_grid_row_kind(pvzp_rs_world* world, std::int32_t row, std::int32_t* kind) noexcept {
    if (!world || !kind) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (row < 0 || row >= MAX_GRID_SIZE_Y) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *kind = static_cast<std::int32_t>(Native(world)->mPlantRow[row]);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_cob_impact_delay(std::uint8_t enabled) noexcept
{
    gModifiers.disableCobImpactDelay = !enabled;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_coins(std::uint32_t* value) noexcept
{
    if (!value || !gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    const std::uint32_t coins = gLawnApp->mPlayerInfo->mCoins;
    if (coins > INT_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *value = coins;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_coins(std::uint32_t value) noexcept
{
    if (!gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (value > INT_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    if (gModifiers.profileReadonly) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
    gLawnApp->mPlayerInfo->mCoins = value;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_pickup_limit(std::uint8_t money, std::uint32_t* value) noexcept
{
    if (!value) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (money > 1) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *value = money ? gModifiers.moneyLimit : gModifiers.sunlightLimit;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_pickup_limit(std::uint8_t money, std::uint32_t value) noexcept
{
    if (money > 1 || value > INT_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    (money ? gModifiers.moneyLimit : gModifiers.sunlightLimit) = static_cast<std::int32_t>(value);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_profile_writable() noexcept
{
    if (!gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    return gModifiers.profileReadonly ? Fail(PVZP_RS_STATUS_UNSUPPORTED) : Ok();
}
extern "C" pvzp_rs_status pvzp_rs_finished_adventure(std::uint32_t* value) noexcept
{
    if (!value || !gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *value = gLawnApp->mPlayerInfo->mFinishedAdventure; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_finished_adventure(std::uint32_t value) noexcept
{
    const auto status = pvzp_rs_profile_writable(); if (status.code) return status;
    gLawnApp->mPlayerInfo->mFinishedAdventure = value; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_challenge_record(std::uint32_t index, std::uint32_t* value) noexcept
{
    if (!value || !gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (index >= std::size(gLawnApp->mPlayerInfo->mChallengeRecords)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *value = gLawnApp->mPlayerInfo->mChallengeRecords[index]; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_challenge_record(std::uint32_t index, std::uint32_t value) noexcept
{
    const auto status = pvzp_rs_profile_writable(); if (status.code) return status;
    if (index >= std::size(gLawnApp->mPlayerInfo->mChallengeRecords)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gLawnApp->mPlayerInfo->mChallengeRecords[index] = value; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_purchase(std::uint32_t index, std::uint32_t* value) noexcept
{
    if (!value || !gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (index >= std::size(gLawnApp->mPlayerInfo->mPurchases)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *value = gLawnApp->mPlayerInfo->mPurchases[index]; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_purchase(std::uint32_t index, std::uint32_t value) noexcept
{
    const auto status = pvzp_rs_profile_writable(); if (status.code) return status;
    if (index >= std::size(gLawnApp->mPlayerInfo->mPurchases)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gLawnApp->mPlayerInfo->mPurchases[index] = value; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_sync_game_selector(std::uint8_t loading) noexcept
{
    return Guard([&] {
        if (!gLawnApp || gLawnApp->mGameScene != GameScenes::SCENE_MENU || !gLawnApp->mGameSelector) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        gLawnApp->mGameSelector->SyncProfile(loading != 0); return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_mouse(std::int32_t operation, std::int32_t x, std::int32_t y, std::int32_t button) noexcept
{
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mWidgetManager) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (gLawnApp->mShutdown) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        if (operation < 0 || operation > 3) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, operation);
        if (operation == 3) {
            if (!gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
            auto* board = gLawnApp->mBoard;
            if (!board->mCursorObject) return Fail(PVZP_RS_STATUS_NULL_POINTER);
            const auto* cursor = board->mCursorObject.get();
            if (cursor->mCursorType == CursorType::CURSOR_TYPE_PLANT_FROM_BANK
                && (!board->mSeedBank || cursor->mSeedBankIndex < 0 || cursor->mSeedBankIndex >= board->mSeedBank->mNumPackets || cursor->mSeedBankIndex >= SEEDBANK_MAX))
                return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
            if (cursor->mCursorType == CursorType::CURSOR_TYPE_PLANT_FROM_USABLE_COIN
                && !board->mCoins.DataArrayTryToGet(cursor->mCoinID)) return Fail(PVZP_RS_STATUS_NOT_FOUND);
            board->RefreshSeedPacketFromCursor();
            return Ok();
        }
        if (x < 0 || y < 0 || x >= gLawnApp->mWidth || y >= gLawnApp->mHeight
            || (button != 1 && button != -1)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        gLawnApp->mMouseIn = true;
        gLawnApp->mLastUserInputTick = gLawnApp->mLastTimerTime;
        gLawnApp->mWidgetManager->MouseMove(x, y);
        if (operation == 1) gLawnApp->mWidgetManager->MouseDown(x, y, button);
        if (operation == 2) gLawnApp->mWidgetManager->MouseUp(x, y, button);
        // Input can replace Board or run a nested modal loop. No pre-input
        // Board pointer is retained or read after the native operation.
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_sound(std::uint32_t id, std::uint8_t stop) noexcept
{
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mSoundManager) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (id >= Sexy::MAX_SOURCE_SOUNDS) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        if (stop) return gLawnApp->mSoundManager->StopSound(id) ? Ok() : Fail(PVZP_RS_STATUS_NOT_FOUND);
        // The native sound manager owns the auto-release instance and channels.
        auto* instance = gLawnApp->mSoundManager->GetSoundInstance(id);
        if (!instance) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        if (!instance->Play(false, true)) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_water_plant(std::int32_t row, std::int32_t col) noexcept
{
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mBoard || !gLawnApp->mZenGarden || !gLawnApp->mPlayerInfo)
            return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* board = gLawnApp->mBoard;
        auto* garden = gLawnApp->mZenGarden;
        if (gLawnApp->mGameMode != GameMode::GAMEMODE_CHALLENGE_ZEN_GARDEN || garden->mBoard != board
            || (board->mBackground != BackgroundType::BACKGROUND_GREENHOUSE
                && board->mBackground != BackgroundType::BACKGROUND_MUSHROOM_GARDEN
                && board->mBackground != BackgroundType::BACKGROUND_ZOMBIQUARIUM))
            return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        if (row < 0 || col < 0 || row >= ZEN_MAX_GRIDSIZE_Y || col >= ZEN_MAX_GRIDSIZE_X
            || garden->GridToPixelX(col, row) < 0) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        for (Plant* plant : board->mPlants) {
            if (plant->mDead || plant->mRow != row || plant->mPlantCol != col || plant->mPottedPlantIndex < 0) continue;
            if (plant->mPottedPlantIndex >= gLawnApp->mPlayerInfo->mNumPottedPlants
                || plant->mPottedPlantIndex >= MAX_POTTED_PLANTS) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
            const auto& potted = gLawnApp->mPlayerInfo->mPottedPlant[plant->mPottedPlantIndex];
            if (potted.mWhichZenGarden != garden->mGardenType) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
            if (!garden->PlantCanBeWatered(plant)) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
            garden->PlantWatered(plant);
            return Ok();
        }
        return Fail(PVZP_RS_STATUS_NOT_FOUND);
    });
}

extern "C" pvzp_rs_status pvzp_rs_unlock_trophy() noexcept
{
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* profile = gLawnApp->mPlayerInfo;
        profile->mFinishedAdventure = std::max(profile->mFinishedAdventure, 1U);
        for (int i = 0; i < NUM_CHALLENGE_MODES; ++i) {
            const auto& definition = GetChallengeDefinition(i);
            const auto mode = definition.mChallengeMode;
            if (definition.mPage != CHALLENGE_PAGE_SURVIVAL && definition.mPage != CHALLENGE_PAGE_CHALLENGE
                && definition.mPage != CHALLENGE_PAGE_PUZZLE) continue;
            if (gLawnApp->IsSurvivalEndless(mode) || gLawnApp->IsEndlessScaryPotter(mode) || gLawnApp->IsEndlessIZombie(mode)) continue;
            const std::uint32_t required = gLawnApp->IsSurvivalNormal(mode) ? SURVIVAL_NORMAL_FLAGS
                : gLawnApp->IsSurvivalHard(mode) ? SURVIVAL_HARD_FLAGS : 1;
            auto& record = profile->mChallengeRecords[mode - GameMode::GAMEMODE_SURVIVAL_NORMAL_STAGE_1];
            record = std::max(record, required);
        }
        return gLawnApp->EarnedGoldTrophy() ? Ok() : Fail(PVZP_RS_STATUS_UNSUPPORTED);
    });
}

extern "C" pvzp_rs_status pvzp_rs_set_temporary_unlock_enabled(std::uint8_t enabled) noexcept
{
    return Guard([&] {
        if (!gLawnApp || (gLawnApp->mGameScene == SCENE_MENU && (!gLawnApp->mPlayerInfo || !gLawnApp->mGameSelector)))
            return Fail(PVZP_RS_STATUS_NULL_POINTER);
        SetTemporaryUnlock(enabled != 0);
        if (gLawnApp->mGameScene == SCENE_MENU) gLawnApp->mGameSelector->SyncProfile(true);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_generation(std::uint64_t* generation) noexcept
{
    if (!gLawnApp || !generation) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (gLawnApp->mBoard && !gBoardEpoch) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *generation = gLawnApp->mBoard ? gBoardEpoch : 0;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_unlock_hidden_modes() noexcept
{
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mChallengeScreen) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        gLawnApp->mChallengeScreen->mLimboPageUnlocked = true;
        gLawnApp->mChallengeScreen->UpdateButtons();
        return Ok();
    });
}

extern "C" void rsvz_pvzp_restore_all() { PvzpNative::RestoreAll(); }
extern "C" std::uint32_t pvzp_rs_plant_id(const pvzp_rs_plant* object) noexcept
{
    // A live handle already proves ownership and excludes slot reclamation.
    return object->mBoard->mPlants.DataArrayGetID(const_cast<pvzp_rs_plant*>(object));
}

extern "C" std::uint32_t pvzp_rs_zombie_id(const pvzp_rs_zombie* object) noexcept
{
    // A live handle already proves ownership and excludes slot reclamation.
    return object->mBoard->mZombies.DataArrayGetID(const_cast<pvzp_rs_zombie*>(object));
}

extern "C" std::uint32_t pvzp_rs_grid_item_id(const pvzp_rs_grid_item* object) noexcept
{
    // A live handle already proves ownership and excludes slot reclamation.
    return object->mBoard->mGridItems.DataArrayGetID(const_cast<pvzp_rs_grid_item*>(object));
}

extern "C" std::uint32_t pvzp_rs_projectile_id(const pvzp_rs_projectile* object) noexcept
{
    // A live handle already proves ownership and excludes slot reclamation.
    return object->mBoard->mProjectiles.DataArrayGetID(const_cast<pvzp_rs_projectile*>(object));
}

extern "C" std::uint32_t pvzp_rs_item_id(const pvzp_rs_item* object) noexcept
{
    // A live handle already proves ownership and excludes slot reclamation.
    return object->mBoard->mCoins.DataArrayGetID(const_cast<pvzp_rs_item*>(object));
}

extern "C" pvzp_rs_world* pvzp_rs_board_from_live_app() noexcept
{
    // The backend token is lent only inside a live LawnApp ingress.
    return reinterpret_cast<pvzp_rs_world*>(gLawnApp->mBoard);
}

extern "C" pvzp_rs_status pvzp_rs_current_world(pvzp_rs_world** outWorld) noexcept
{
	if (!outWorld)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (!gLawnApp || !gLawnApp->mBoard)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outWorld = reinterpret_cast<pvzp_rs_world*>(gLawnApp->mBoard);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_game_ui(std::int32_t* outGameUi) noexcept
{
	if (!outGameUi)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (!gLawnApp)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outGameUi = static_cast<std::int32_t>(gLawnApp->mGameScene);
	return Ok();
}

#define PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(name, type, expression) \
	extern "C" pvzp_rs_status pvzp_rs_seed_chooser_##name(type* outValue) noexcept \
	{ \
		if (!outValue) \
			return Fail(PVZP_RS_STATUS_NULL_POINTER); \
		SeedChooserScreen* chooser = CurrentSeedChooser(); \
		if (!chooser) \
			return Fail(PVZP_RS_STATUS_NOT_FOUND); \
		*outValue = (expression); \
		return Ok(); \
	}

PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(mouse_visible, std::uint8_t, static_cast<std::uint8_t>(chooser->mMouseVisible))
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(parent_present, std::uint8_t, static_cast<std::uint8_t>(chooser->mParent != nullptr))
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(widget_manager_present, std::uint8_t, static_cast<std::uint8_t>(chooser->mWidgetManager != nullptr))
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(modal_present, std::uint8_t, static_cast<std::uint8_t>(chooser->mWidgetManager && chooser->mWidgetManager->mBaseModalWidget))
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(choose_state, std::int32_t, static_cast<std::int32_t>(chooser->mChooseState))
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(view_lawn_time, std::int32_t, chooser->mViewLawnTime)
PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR(seeds_in_flight, std::int32_t, chooser->mSeedsInFlight)

#undef PVZP_RS_DEFINE_SEED_CHOOSER_SCALAR

extern "C" pvzp_rs_status pvzp_rs_seed_chooser_cancel_view_lawn() noexcept
{
	SeedChooserScreen* chooser = CurrentSeedChooser();
	if (!chooser || chooser->mChooseState != SeedChooserState::CHOOSE_VIEW_LAWN)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	chooser->mViewLawnTime = 251;
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_chooser_card_available(std::int32_t kind, std::uint8_t* available) noexcept
{
    return Guard([&] {
        auto* chooser = CurrentSeedChooser();
        if (!available || !chooser || !gLawnApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        if (kind < 0 || kind > SEED_IMITATER) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, kind);
        const auto seed = static_cast<SeedType>(kind);
        *available = static_cast<std::uint8_t>(gLawnApp->HasSeedType(seed)
            && !chooser->SeedNotAllowedToPick(seed) && !chooser->SeedNotAllowedDuringTrial(seed));
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_chooser_card_locked(std::uint32_t index, std::uint8_t* locked) noexcept
{
    return Guard([&] {
        auto* chooser = CurrentSeedChooser();
        if (!chooser || !locked) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        if (index >= static_cast<std::uint32_t>(chooser->mSeedsInBank)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        const auto kind = chooser->FindSeedInBank(static_cast<int>(index));
        if (kind < 0 || kind > SEED_IMITATER) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        *locked = static_cast<std::uint8_t>(chooser->mChosenSeeds[kind].mCrazyDavePicked);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_deselect_card(std::uint32_t index) noexcept
{
    return Guard([&] {
        auto* chooser = CurrentSeedChooser();
        if (!chooser) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        if (index >= static_cast<std::uint32_t>(chooser->mSeedsInBank)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        const auto kind = chooser->FindSeedInBank(static_cast<int>(index));
        if (kind < 0 || kind > SEED_IMITATER) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        auto& seed = chooser->mChosenSeeds[kind];
        if (seed.mSeedState != ChosenSeedState::SEED_IN_BANK || seed.mCrazyDavePicked)
            return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        chooser->ClickedSeedInBank(seed);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_stop_seed_chooser_fast_forward() noexcept
{
    PvzpNative::gSeedChooserFastForwardRemaining = 0;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_click_continue_dialog_if_present(std::uint8_t* outClicked) noexcept
{
	return Guard([&] {
		if (!outClicked)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		*outClicked = static_cast<std::uint8_t>(gLawnApp->KillDialog(Dialogs::DIALOG_CONTINUE));
		return Ok();
	});
}

extern "C" std::uint8_t pvzp_rs_key_is_down(std::uint16_t key) noexcept
{
    return gLawnApp && gLawnApp->mWidgetManager && key < 0xFF
        && gLawnApp->mWidgetManager->mKeyDown[key];
}

extern "C" pvzp_rs_status pvzp_rs_input_focused(std::uint8_t* outFocused) noexcept
{
	return Guard([&] {
		if (!outFocused)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		*outFocused = static_cast<std::uint8_t>(PvzpNative::InputFocused());
		return Ok();
	});
}

#define PVZP_RS_DEFINE_WORLD_SCALAR(name, type, expression) \
    extern "C" type pvzp_rs_world_##name(const pvzp_rs_world* world) noexcept \
    { return (expression); }

PVZP_RS_DEFINE_WORLD_SCALAR(scene, std::int32_t, static_cast<std::int32_t>(Native(world)->mBackground))
PVZP_RS_DEFINE_WORLD_SCALAR(paused, std::uint8_t, static_cast<std::uint8_t>(Native(world)->mPaused))
PVZP_RS_DEFINE_WORLD_SCALAR(main_counter, std::uint32_t, Native(world)->mMainCounter)
PVZP_RS_DEFINE_WORLD_SCALAR(current_wave, std::int32_t, Native(world)->mCurrentWave)
PVZP_RS_DEFINE_WORLD_SCALAR(sun, std::int32_t, Native(world)->mSunMoney)
PVZP_RS_DEFINE_WORLD_SCALAR(level_complete, std::uint8_t, static_cast<std::uint8_t>(Native(world)->mLevelComplete))
PVZP_RS_DEFINE_WORLD_SCALAR(next_survival_stage_counter, std::int32_t, Native(world)->mNextSurvivalStageCounter)
PVZP_RS_DEFINE_WORLD_SCALAR(frame_duration_ms, std::int32_t, Native(world)->mApp->mFrameTime)
PVZP_RS_DEFINE_WORLD_SCALAR(survival_stage, std::int32_t, Native(world)->mChallenge->mSurvivalStage)
// The original method is not const-qualified but only reads Board/Challenge.
PVZP_RS_DEFINE_WORLD_SCALAR(survival_flags_completed, std::int32_t, const_cast<Board*>(Native(world))->GetSurvivalFlagsCompleted())

PVZP_RS_DEFINE_WORLD_SCALAR(total_waves, std::int32_t, Native(world)->mNumWaves)
PVZP_RS_DEFINE_WORLD_SCALAR(refresh_countdown, std::int32_t, Native(world)->mZombieCountDown)
PVZP_RS_DEFINE_WORLD_SCALAR(initial_countdown, std::int32_t, Native(world)->mZombieCountDownStart)
PVZP_RS_DEFINE_WORLD_SCALAR(huge_wave_countdown, std::int32_t, Native(world)->mHugeWaveCountDown)
PVZP_RS_DEFINE_WORLD_SCALAR(level_end_countdown, std::int32_t, Native(world)->mFinalWaveSoundCounter)

#undef PVZP_RS_DEFINE_WORLD_SCALAR

extern "C" pvzp_rs_status pvzp_rs_world_set_survival_stage(pvzp_rs_world* world, std::int32_t stage) noexcept {
    if (!world) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (stage < -1 || stage > (INT_MAX / 2 - 110) / 20) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, stage);
    Native(world)->mChallenge->mSurvivalStage = stage;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_set_pause_dialog(pvzp_rs_world* world, std::uint8_t enabled) noexcept {
    return Guard([&] {
        if (!world || !gLawnApp) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING || Native(world) != gLawnApp->mBoard)
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        if (Native(world)->mPaused == (enabled != 0)) return Ok();
        if (enabled) {
            gLawnApp->PlaySample(SOUND_PAUSE);
            gLawnApp->DoPauseDialog();
        } else {
            gLawnApp->ButtonDepress(2000 + Dialogs::DIALOG_PAUSED);
        }
        return Ok();
    });
}
extern "C" pvzp_rs_status pvzp_rs_world_begin_level_completion(pvzp_rs_world* world) noexcept {
    return Guard([&] {
        const auto checked = pvzp_rs_world_validate_level_completion(world);
        if (checked.code != PVZP_RS_STATUS_OK) return checked;
        Native(world)->FadeOutLevel();
        return Ok();
    });
}
extern "C" pvzp_rs_status pvzp_rs_world_validate_level_completion(const pvzp_rs_world* world) noexcept {
    if (!world || !gLawnApp) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    const auto* board = Native(world);
    if (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING || board != gLawnApp->mBoard)
        return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    if (gLawnApp->IsSurvivalMode()) {
        const auto* bank = board->mSeedBank.get();
        if (!bank || bank->mNumPackets < 0 || bank->mNumPackets > SEEDBANK_MAX) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        for (int index = 0; index < bank->mNumPackets; ++index) {
            const auto& packet = bank->mSeedPackets[index];
            const auto kind = static_cast<int>(packet.mPacketType);
            const auto target = static_cast<int>(packet.mImitaterType);
            if (kind < 0 || kind > 52 || (kind == 48 && (target < 0 || target > 52))) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, index);
        }
    }
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_frame_duration_ms(std::int32_t* outValue) noexcept {
    if (!gLawnApp || !outValue) return {PVZP_RS_STATUS_NULL_POINTER, 0};
    *outValue = gLawnApp->mFrameTime;
    return {PVZP_RS_STATUS_OK, 0};
}

extern "C" pvzp_rs_status pvzp_rs_world_show_hint(pvzp_rs_world* world, const char* text, std::uint32_t length, std::int32_t duration) noexcept {
    return Guard([&] {
        if (!world || !text || !Native(world)->mAdvice) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (length > 126 || duration < 0 || std::memchr(text, 0, length) || (length >= 3 && text[0] == '['))
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        auto* words = Native(world)->mAdvice.get();
        words->SetLabel(std::string_view(text, length), MessageStyle::MESSAGE_STYLE_HINT_FAST);
        words->mDuration = duration;
        return Ok();
    });
}
extern "C" pvzp_rs_status pvzp_rs_world_caption_text(const pvzp_rs_world* world, char* output, std::uint32_t capacity, std::uint32_t* length) noexcept {
    if (!world || !output || !length || !Native(world)->mAdvice) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    const auto& label = Native(world)->mAdvice->mLabel;
    std::uint32_t size = 0;
    while (size < sizeof(label) && label[size]) ++size;
    if (size == sizeof(label) || size > capacity) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    std::memcpy(output, label, size); *length = size; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_caption_duration(const pvzp_rs_world* world, std::int32_t* value) noexcept {
    if (!world || !value || !Native(world)->mAdvice) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *value = Native(world)->mAdvice->mDuration; return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_caption_style(const pvzp_rs_world* world, std::int32_t* value) noexcept {
    if (!world || !value || !Native(world)->mAdvice) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *value = static_cast<int>(Native(world)->mAdvice->mMessageStyle); return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_seed_choosing(const pvzp_rs_world* world, std::uint8_t* outChoosing) noexcept
{
	if (!world || !outChoosing)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	CutScene* cutScene = Native(world)->mCutScene.get();
	if (!cutScene)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outChoosing = static_cast<std::uint8_t>(cutScene->mSeedChoosing);
	return Ok();
}

extern "C" std::uint8_t pvzp_rs_world_imitater_successor(
    const pvzp_rs_world*, std::uint32_t placeholderId, std::uint32_t* outSuccessorId) noexcept
{
    const bool found = placeholderId != 0 && placeholderId == gMorphPlaceholder;
    *outSuccessorId = found ? gMorphSuccessor : 0;
    return static_cast<std::uint8_t>(found);
}

extern "C" pvzp_rs_status pvzp_rs_world_set_sun(pvzp_rs_world* world, std::uint32_t sun) noexcept
{
	if (!world)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (sun > static_cast<std::uint32_t>(INT32_MAX))
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, INT32_MAX);
	Native(world)->mSunMoney = static_cast<std::int32_t>(sun);
	return Ok();
}

extern "C" std::int32_t pvzp_rs_world_natural_sun_generated(const pvzp_rs_world* world) noexcept { return Native(world)->mNumSunsFallen; }

extern "C" std::int32_t pvzp_rs_world_natural_sun_countdown(const pvzp_rs_world* world) noexcept { return Native(world)->mSunCountDown; }

extern "C" pvzp_rs_status pvzp_rs_world_set_natural_sun_generated(pvzp_rs_world* world, std::int32_t count) noexcept
{
	if (!world) return Fail(PVZP_RS_STATUS_NULL_POINTER); Native(world)->mNumSunsFallen = count; return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_set_natural_sun_countdown(pvzp_rs_world* world, std::int32_t countdown) noexcept
{
	if (!world) return Fail(PVZP_RS_STATUS_NULL_POINTER); Native(world)->mSunCountDown = countdown; return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_app_counter(std::uint32_t* outCounter) noexcept
{
	if (!outCounter)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (!gLawnApp)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outCounter = static_cast<std::uint32_t>(gLawnApp->mAppCounter);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_app_counter(std::uint32_t counter) noexcept
{
	if (!gLawnApp)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	gLawnApp->mAppCounter = static_cast<std::int32_t>(counter);
	return Ok();
}

#define PVZP_RS_DEFINE_WORLD_ROW_VALUE(name, type, writeValue) \
	extern "C" pvzp_rs_status pvzp_rs_world_##name(const pvzp_rs_world* world, std::uint32_t row, type* outValue) noexcept \
	{ \
		if (!world || !outValue) return Fail(PVZP_RS_STATUS_NULL_POINTER); \
		if (row >= MAX_GRID_SIZE_Y) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(row)); \
		writeValue; \
		return Ok(); \
	}

static_assert(sizeof(float) == sizeof(std::uint32_t));
PVZP_RS_DEFINE_WORLD_ROW_VALUE(ice_path_x, std::int32_t, *outValue = Native(world)->mIceMinX[row])
PVZP_RS_DEFINE_WORLD_ROW_VALUE(ice_path_countdown, std::uint32_t, *outValue = static_cast<std::uint32_t>(Native(world)->mIceTimer[row]))
PVZP_RS_DEFINE_WORLD_ROW_VALUE(row_pick_weight_bits, std::uint32_t, std::memcpy(outValue, &Native(world)->mRowPickingArray[row].mWeight, sizeof(float)))
PVZP_RS_DEFINE_WORLD_ROW_VALUE(row_pick_last_picked_bits, std::uint32_t, std::memcpy(outValue, &Native(world)->mRowPickingArray[row].mLastPicked, sizeof(float)))
PVZP_RS_DEFINE_WORLD_ROW_VALUE(row_pick_second_last_picked_bits, std::uint32_t, std::memcpy(outValue, &Native(world)->mRowPickingArray[row].mSecondLastPicked, sizeof(float)))

#undef PVZP_RS_DEFINE_WORLD_ROW_VALUE

extern "C" pvzp_rs_status pvzp_rs_world_spawn_allowed(const pvzp_rs_world* world, std::uint32_t zombieType, std::uint8_t* outAllowed) noexcept
{
	if (!world || !outAllowed)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (zombieType >= static_cast<std::uint32_t>(ZombieType::NUM_ZOMBIE_TYPES))
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(zombieType));
	*outAllowed = static_cast<std::uint8_t>(Native(world)->mZombieAllowed[zombieType]);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_spawn_entry(const pvzp_rs_world* world, std::uint32_t wave, std::uint32_t slot, std::int32_t* outZombieType) noexcept
{
	if (!world || !outZombieType)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (wave >= MAX_ZOMBIE_WAVES || slot >= MAX_ZOMBIES_IN_WAVE)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(wave));
	*outZombieType = static_cast<std::int32_t>(Native(world)->mZombiesInWave[wave][slot]);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_commit_wave_refresh(pvzp_rs_world* world, std::int32_t expectedWave, std::int32_t initialCountdown) noexcept
{
	if (!world)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (initialCountdown <= 0)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, initialCountdown);
	auto* board = Native(world);
	if (board->mCurrentWave != expectedWave)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, board->mCurrentWave);
	board->mZombieHealthToNextWave = -1;
	board->mZombieCountDown = initialCountdown - 1;
	board->mZombieCountDownStart = initialCountdown;
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_zombie_health_wave_start(const pvzp_rs_world* world, std::int32_t* outHealth) noexcept
{
	if (!world || !outHealth) return Fail(PVZP_RS_STATUS_NULL_POINTER); *outHealth = Native(world)->mZombieHealthWaveStart; return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_total_zombie_health_in_wave(pvzp_rs_world* world, std::int32_t wave, std::int32_t* outHealth) noexcept
{
	return Guard([&] {
		if (!world || !outHealth)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (wave < 0 || wave >= Native(world)->mNumWaves)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, wave);
		*outHealth = Native(world)->TotalZombiesHealthInWave(wave);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_validate_scene_layout(pvzp_rs_world* world,std::int32_t scene) noexcept {
    if (!world || !gLawnApp || Native(world)!=gLawnApp->mBoard || !gLawnApp->mMusic || !gLawnApp->mEffectSystem) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (scene<0 || scene>9) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    // Scene edits run at an exclusive outer boundary; a modal does not invalidate the Board.
    if (gLawnApp->mGameScene!=GameScenes::SCENE_PLAYING && gLawnApp->mGameScene!=GameScenes::SCENE_LEVEL_INTRO) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_set_scene_layout(pvzp_rs_world* world,std::int32_t scene) noexcept {
    return Guard([&] {
        const auto valid=pvzp_rs_world_validate_scene_layout(world,scene);if(valid.code!=0)return valid;
        auto* board=Native(world);const bool pool=scene==2||scene==3;
        board->mBackground=static_cast<BackgroundType>(scene);board->LoadBackgroundImages();
        for(int row=0;row<MAX_GRID_SIZE_Y;++row) {
            const bool water=pool&&(row==2||row==3);
            board->mPlantRow[row]=water?PlantRowType::PLANTROW_POOL:row==5&&!pool?PlantRowType::PLANTROW_DIRT:PlantRowType::PLANTROW_NORMAL;
            for(int col=0;col<MAX_GRID_SIZE_X;++col)board->mGridSquareType[col][row]=water?GridSquareType::GRIDSQUARE_POOL:row==5&&!pool?GridSquareType::GRIDSQUARE_DIRT:GridSquareType::GRIDSQUARE_GRASS;
        }
        if(!pool) {
            if(auto* holder=gLawnApp->mEffectSystem->mParticleHolder.get())
                for(auto* particle:holder->mParticleSystems)if(!particle->mDead&&static_cast<int>(particle->mEffectType)==34)particle->ParticleSystemDie();
            board->mPoolSparklyParticleID=ParticleSystemID::PARTICLESYSTEMID_NULL;
        }
        gLawnApp->mMusic->PlayMusic(static_cast<MusicTune>(scene==5?2:scene>=6?8:scene+1));
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_set_scene(pvzp_rs_world* world, std::int32_t scene) noexcept
{
	return Guard([&] {
		Board* board = Native(world);
		if (!board || !gLawnApp || board != gLawnApp->mBoard)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (scene < static_cast<std::int32_t>(BackgroundType::BACKGROUND_1_DAY)
			|| scene > static_cast<std::int32_t>(BackgroundType::BACKGROUND_MUSHROOM_GARDEN))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, scene);
		if (!gLawnApp->IsSurvivalMode() || !board->mChallenge || !board->mCutScene)
			return Fail(PVZP_RS_STATUS_UNSUPPORTED);

		const BackgroundType background = static_cast<BackgroundType>(scene);
		const bool hasPool = background == BackgroundType::BACKGROUND_3_POOL
			|| background == BackgroundType::BACKGROUND_4_FOG;
		const bool hadLawnMowers = board->mLawnMowers.mSize > 0;

		board->mBackground = background;
		board->LoadBackgroundImages();
		for (int row = 0; row < MAX_GRID_SIZE_Y; ++row)
		{
			const bool poolRow = hasPool && (row == 2 || row == 3);
			board->mPlantRow[row] = poolRow ? PlantRowType::PLANTROW_POOL
				: row == 5 && !hasPool ? PlantRowType::PLANTROW_DIRT : PlantRowType::PLANTROW_NORMAL;
			for (int col = 0; col < MAX_GRID_SIZE_X; ++col)
				board->mGridSquareType[col][row] = poolRow ? GridSquareType::GRIDSQUARE_POOL
					: row == 5 && !hasPool ? GridSquareType::GRIDSQUARE_DIRT : GridSquareType::GRIDSQUARE_GRASS;
		}

		if (!hasPool)
		{
			if (PvzpParticleSystem* particle = gLawnApp->ParticleTryToGet(board->mPoolSparklyParticleID))
				particle->ParticleSystemDie();
			board->mPoolSparklyParticleID = ParticleSystemID::PARTICLESYSTEMID_NULL;
		}

		if (hadLawnMowers)
			ClearLawnMowers(board);

		--board->mChallenge->mSurvivalStage;
		board->mCutScene->mPlacedLawnItems = false;
		board->mChallenge->PuzzleNextStageClear();
		board->mCutScene->mPlacedLawnItems = true;
		const MusicTune tune = (background == BackgroundType::BACKGROUND_6_BOSS
			|| background == BackgroundType::BACKGROUND_MUSHROOM_GARDEN)
			? MusicTune::MUSIC_TUNE_NIGHT_MOONGRAINS
			: static_cast<MusicTune>(scene + 1);
		gLawnApp->mMusic->PlayMusic(tune);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_clear_lawn_mowers(pvzp_rs_world* world) noexcept
{
	return Guard([&] {
		if (!world)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		ClearLawnMowers(Native(world));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_set_spawn_allowed(pvzp_rs_world* world, std::int32_t zombieType, std::uint8_t allowed) noexcept
{
	return Guard([&] {
		if (!world)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!IsZombieType(zombieType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, zombieType);
		Native(world)->mZombieAllowed[zombieType] = allowed != 0;
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_set_spawn_entry(pvzp_rs_world* world, std::uint32_t wave, std::uint32_t slot, std::int32_t zombieType) noexcept
{
	return Guard([&] {
		if (!world)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (wave >= MAX_ZOMBIE_WAVES || slot >= MAX_ZOMBIES_IN_WAVE
			|| (zombieType != static_cast<std::int32_t>(ZombieType::ZOMBIE_INVALID) && !IsZombieType(zombieType)))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, zombieType);
		Native(world)->mZombiesInWave[wave][slot] = static_cast<ZombieType>(zombieType);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_pick_spawn_list(pvzp_rs_world* world) noexcept
{
	return Guard([&] {
		if (!world || !Native(world)->mChallenge) return Fail(PVZP_RS_STATUS_NULL_POINTER);
		SpawnStageScope stage(Native(world));
		Native(world)->PickZombieWaves();
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_spawn_huge_wave(pvzp_rs_world* world,std::uint32_t wave,std::uint8_t* huge) noexcept {
    if (!world || !huge) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    auto* board=Native(world);
    if (board->mNumWaves<=0 || board->mNumWaves>PVZP_RS_SPAWN_WAVE_CAPACITY || wave>=static_cast<unsigned>(board->mNumWaves)) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *huge=board->IsFlagWave(static_cast<int>(wave));return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_refresh_spawn_preview(pvzp_rs_world* world) noexcept {
    return Guard([&]{
        if (!world || !gLawnApp || gLawnApp->mBoard!=Native(world)) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (gLawnApp->mGameScene!=GameScenes::SCENE_LEVEL_INTRO) return Ok();
        auto* board=Native(world);if (!board->mCutScene) return Fail(PVZP_RS_STATUS_NOT_FOUND);
        board->RemoveCutsceneZombies();board->mCutScene->mPlacedZombies=false;return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_board_spawn_seed(pvzp_rs_world* world,std::uint32_t* seed) noexcept {
    if (!world || !seed) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *seed=static_cast<std::uint32_t>(Native(world)->mBoardRandSeed);return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_generate_current_spawn(pvzp_rs_world* world,std::uint32_t seed) noexcept {
    return Guard([&]{
        if (!world || !gLawnApp || gLawnApp->mBoard!=Native(world) || !Native(world)->mChallenge) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (gLawnApp->mGameScene!=GameScenes::SCENE_LEVEL_INTRO && gLawnApp->mGameScene!=GameScenes::SCENE_PLAYING) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        SpawnStageScope stage(Native(world));
        PvzpNative::BuildCurrentZombieWaves(Native(world),seed);return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_start_battle() noexcept
{
	return Guard([&] {
		SeedChooserScreen* chooser = CurrentSeedChooser();
		if (!chooser || !gLawnApp->mBoard || !gLawnApp->mBoard->mCutScene || !gLawnApp->mBoard->mSeedBank)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		const int selected = chooser->mSeedsInBank;
		const int slots = gLawnApp->mBoard->mSeedBank->mNumPackets;
		if (selected > slots)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, selected);
		if (selected < slots && gRandomStreams[0].locked)
			return Fail(PVZP_RS_STATUS_UNSUPPORTED);
		if (gPendingSurvivalStage >= 0 && gLawnApp->mBoard && gLawnApp->mBoard->mChallenge)
			gLawnApp->mBoard->mChallenge->mSurvivalStage = gPendingSurvivalStage;
		chooser->PickRandomSeeds();
		if (gPendingReadyCooldowns && gLawnApp->mBoard && gLawnApp->mBoard->mSeedBank)
		{
			SeedBank* bank = gLawnApp->mBoard->mSeedBank.get();
			for (int i = 0; i < bank->mNumPackets; ++i)
				bank->mSeedPackets[i].mRefreshCounter = bank->mSeedPackets[i].mRefreshTime;
		}
		gPendingSurvivalStage = -1;
		gPendingReadyCooldowns = false;
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_enter_native_mode(std::int32_t modeValue) noexcept
{
	return Guard([&] {
		if (!gLawnApp || !gLawnApp->mPlayerInfo)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		const GameMode mode = static_cast<GameMode>(modeValue);
		if (modeValue < 0 || modeValue > 73)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, modeValue);
		switch (gLawnApp->mGameScene)
		{
		case GameScenes::SCENE_LOADING:
			if (!gLawnApp->mTitleScreen || !gLawnApp->mTitleScreen->mLoadingThreadComplete)
				return Fail(PVZP_RS_STATUS_NOT_FOUND);
			gLawnApp->LoadingCompleted();
			gLawnApp->KillGameSelector();
			break;
		case GameScenes::SCENE_MENU:
			gLawnApp->KillGameSelector();
			break;
		case GameScenes::SCENE_CHALLENGE:
			gLawnApp->KillChallengeScreen();
			break;
		default:
			return Fail(PVZP_RS_STATUS_UNSUPPORTED, static_cast<std::int32_t>(gLawnApp->mGameScene));
		}
		gLawnApp->PreNewGame(mode, true);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_enter_endless(std::int32_t scene) noexcept {
    const GameMode mode=EndlessMode(scene);
    if (mode==GameMode::NUM_GAME_MODES) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT,scene);
    return pvzp_rs_enter_native_mode(static_cast<std::int32_t>(mode));
}
extern "C" pvzp_rs_status pvzp_rs_save_current_game() noexcept {
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mBoard || gLawnApp->mGameScene != GameScenes::SCENE_PLAYING)
            return Fail(PVZP_RS_STATUS_NOT_FOUND);
        gLawnApp->mBoard->TryToSaveGame();
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_back_to_main_menu() noexcept
{
	return Guard([&] {
		if (!gLawnApp || !gLawnApp->mBoard || gLawnApp->mGameScene != GameScenes::SCENE_PLAYING)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		gLawnApp->DoBackToMain();
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_reset(
	pvzp_rs_world* world, std::uint32_t completedRounds, std::uint32_t seed,
	std::uint32_t initialSun, std::uint8_t readyCooldowns) noexcept
{
	return Guard([&] {
		if (!world || !gLawnApp || Native(world) != gLawnApp->mBoard || !gLawnApp->mPlayerInfo)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp->IsSurvivalMode() || !Native(world)->mChallenge)
			return Fail(PVZP_RS_STATUS_UNSUPPORTED);
		if (completedRounds > static_cast<std::uint32_t>(INT32_MAX) || initialSun > static_cast<std::uint32_t>(INT32_MAX))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
		RandomConstructionGuard randomConstruction(seed);
		gPendingSurvivalStage = -1;
		gPendingReadyCooldowns = false;
		gLawnApp->mAppRandSeed = seed;
		Sexy::SRand(seed);
		// Like native Try Again, dismiss the old modal before replacing its Board.
		gLawnApp->KillDialog(Dialogs::DIALOG_GAME_OVER);
		gLawnApp->NewGame();
		Board* board = gLawnApp->mBoard;
		if (!board || !board->mChallenge)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		const std::uint32_t offset = static_cast<std::uint32_t>(gLawnApp->mPlayerInfo->mId)
			+ completedRounds * 101U + static_cast<std::uint32_t>(gLawnApp->mGameMode);
		board->mSunMoney = static_cast<std::int32_t>(initialSun);
		board->mBoardRandSeed = static_cast<std::int32_t>(seed - offset);
		board->mChallenge->mSurvivalStage = static_cast<std::int32_t>(completedRounds);
		board->InitZombieWaves();
		board->mChallenge->mSurvivalStage = 0;
		gLawnApp->mAppCounter = static_cast<std::int32_t>(MixDancerClock(seed));
		gPendingDancerClock = MixDancerClock(seed);
		gPendingWorldOrigin = true;
		gPendingCompletedRounds = 0;
		gPendingSurvivalStage = static_cast<std::int32_t>(completedRounds);
		gPendingReadyCooldowns = readyCooldowns != 0;
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_set_fast_forward(
	std::uint8_t enabled, std::int32_t performance, std::uint8_t suppressWindow) noexcept
{
	if (performance < 0 || performance > 2)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, performance);
	if (enabled && gAdvancedPause)
		return Fail(PVZP_RS_STATUS_UNSUPPORTED);
	gFastForward = enabled != 0;
	gFastForwardPerformance = enabled ? performance : 0;
	gFastForwardSuppressWindow = enabled && suppressWindow != 0;
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_game_speed_multiplier(double* value) noexcept
{
    if (!gLawnApp || !value) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *value = gLawnApp->mUpdateMultiplier;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_game_speed_multiplier(double value) noexcept
{
    if (!gLawnApp) return Fail(PVZP_RS_STATUS_NOT_FOUND);
    if (!std::isfinite(value) || value < 0) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    if (!gGameSpeedCaptured) { gOriginalUpdateMultiplier = gLawnApp->mUpdateMultiplier; gGameSpeedCaptured = true; }
    gLawnApp->mUpdateMultiplier = value;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_game_speed(float speed) noexcept
{
    if (!std::isfinite(speed) || speed < 0.05f || speed > 100.0f) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    return pvzp_rs_set_game_speed_multiplier(speed);
}

extern "C" pvzp_rs_status pvzp_rs_restore_game_speed() noexcept
{
	return Guard([&] { RestoreGameSpeed(); return Ok(); });
}

extern "C" std::uint8_t pvzp_rs_fast_forward_active() noexcept { return static_cast<std::uint8_t>(gFastForward); }

extern "C" pvzp_rs_status pvzp_rs_request_seed_chooser_fast_forward(std::uint32_t maxFrames) noexcept
{
	if (maxFrames == 0)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
	if (gAdvancedPause)
		return Fail(PVZP_RS_STATUS_UNSUPPORTED);
	gFastForward = false;
	gFastForwardPerformance = 0;
	gFastForwardSuppressWindow = false;
	gSeedChooserFastForwardRemaining = maxFrames;
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_advanced_pause(
	std::uint8_t enabled, std::uint8_t drawMask, std::uint32_t rgba,
	std::uint8_t playSound, std::uint8_t refreshCursorPreview) noexcept
{
	return Guard([&] {
		if (enabled && gFastForward)
			return Fail(PVZP_RS_STATUS_UNSUPPORTED);
		SetAdvancedPause(enabled != 0);
		gAdvancedPauseDrawMask = drawMask != 0;
		gAdvancedPauseMaskRgba = rgba;
		gAdvancedPauseRefreshCursor = refreshCursorPreview != 0;
		if (playSound && gLawnApp)
			gLawnApp->PlaySample(SOUND_PAUSE);
		return Ok();
	});
}

extern "C" void pvzp_rs_advanced_pause_options(std::uint8_t* drawMask, std::uint32_t* rgba, std::uint8_t* cursor) noexcept
{
    *drawMask = static_cast<std::uint8_t>(gAdvancedPauseDrawMask);
    *rgba = gAdvancedPauseMaskRgba;
    *cursor = static_cast<std::uint8_t>(gAdvancedPauseRefreshCursor);
}
extern "C" std::uint8_t pvzp_rs_advanced_pause_active() noexcept { return static_cast<std::uint8_t>(gAdvancedPause); }

extern "C" pvzp_rs_status pvzp_rs_release_advanced_pause_for_step(std::uint8_t playSound) noexcept
{
    // The core owns completion/restoration. This temporary logic bypass is not a
    // user-visible resume and must not resume Android video recording.
    return Guard([&] {
        gAdvancedPause = false;
        if (playSound && gLawnApp)
            gLawnApp->PlaySample(SOUND_PAUSE);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_set_random_mode(std::int32_t mode, std::uint32_t value) noexcept
{
	return Guard([&] {
		if (mode < 0 || mode > 1)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, mode);
		for (RandomState& state : gRandomStreams)
			state.Reset(mode, value);
		if (mode == 0)
			Sexy::SRand(value == 0 ? 4357 : value);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_set_wave_spawn_random_seed(std::uint8_t enabled, std::uint32_t seed) noexcept
{
	if (gWaveSpawnActive || gWaveRowPickActive)
		return Fail(PVZP_RS_STATUS_UNSUPPORTED);
	gWaveSpawnRandomEnabled = enabled != 0;
	gWaveSpawnRandomSeed = seed;
	return Ok();
}

#define PVZP_RS_DEFINE_RANDOM_SCALAR(name, type, expression) \
	extern "C" pvzp_rs_status pvzp_rs_random_##name(std::int32_t stream, type* outValue) noexcept \
	{ \
		return Guard([&] { \
			if (!outValue) return Fail(PVZP_RS_STATUS_NULL_POINTER); \
			if (stream < 0 || stream > 1) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, stream); \
			const RandomState& state = gRandomStreams[stream]; \
			*outValue = (expression); \
			return Ok(); \
		}); \
	}

PVZP_RS_DEFINE_RANDOM_SCALAR(seed, std::uint32_t, stream == 1 && gLawnApp && gLawnApp->mBoard ? static_cast<std::uint32_t>(gLawnApp->mBoard->GetLevelRandSeed()) : state.seed)
extern "C" std::uint8_t pvzp_rs_random_locked(bool level_stream) noexcept { return static_cast<std::uint8_t>(gRandomStreams[level_stream ? 1 : 0].locked); }
extern "C" std::uint32_t pvzp_rs_random_fixed(bool level_stream) noexcept { return gRandomStreams[level_stream ? 1 : 0].fixed; }

#undef PVZP_RS_DEFINE_RANDOM_SCALAR

extern "C" pvzp_rs_status pvzp_rs_set_sun_production_mode(std::int32_t mode) noexcept
{
	if (mode < 0 || mode > 1)
		return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, mode);
	gDirectSunProduction = mode == 0;
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_cursor_type(const pvzp_rs_world* world, std::int32_t* outCursorType) noexcept
{
	if (!world || !outCursorType)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	const auto* cursor = Native(world)->mCursorObject.get();
	if (!cursor)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outCursorType = static_cast<std::int32_t>(cursor->mCursorType);
	return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_cursor_seed_kind(const pvzp_rs_world* world, std::int32_t* outValue) noexcept {
    if (!world || !outValue || !Native(world)->mCursorObject) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *outValue = static_cast<std::int32_t>(Native(world)->mCursorObject->mType);
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_world_cursor_imitater_kind(const pvzp_rs_world* world, std::int32_t* outValue) noexcept {
    if (!world || !outValue || !Native(world)->mCursorObject) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *outValue = static_cast<std::int32_t>(Native(world)->mCursorObject->mImitaterType);
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_has_conveyor(const pvzp_rs_world* world, std::uint8_t* outValue) noexcept
{
	if (!world || !outValue) return Fail(PVZP_RS_STATUS_NULL_POINTER);
	*outValue = static_cast<std::uint8_t>(const_cast<Board*>(Native(world))->HasConveyorBeltSeedBank());
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_is_column_challenge(const pvzp_rs_world* world, std::uint8_t* outValue) noexcept
{
	if (!world || !outValue) return Fail(PVZP_RS_STATUS_NULL_POINTER);
	*outValue = static_cast<std::uint8_t>(Native(world)->mApp->mGameMode == GameMode::GAMEMODE_CHALLENGE_COLUMN);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_is_wallnut_bowling(const pvzp_rs_world* world, std::uint8_t* outValue) noexcept
{
	if (!world || !outValue) return Fail(PVZP_RS_STATUS_NULL_POINTER);
	*outValue = static_cast<std::uint8_t>(Native(world)->mApp->IsWallnutBowlingLevel());
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_grid_to_pixel_x(const pvzp_rs_world* world, std::int32_t gridX, std::int32_t gridY, std::int32_t* outX) noexcept
{
	return Guard([&] {
		if (!world || !outX)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outX = const_cast<Board*>(Native(world))->GridToPixelX(gridX, gridY);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_grid_to_pixel_y(const pvzp_rs_world* world, std::int32_t gridX, std::int32_t gridY, std::int32_t* outY) noexcept
{
	return Guard([&] {
		if (!world || !outY)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outY = const_cast<Board*>(Native(world))->GridToPixelY(gridX, gridY);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_pos_y_based_on_row(const pvzp_rs_world* world, float posX, std::int32_t row, float* outY) noexcept
{
	return Guard([&] {
		if (!world || !outY)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outY = const_cast<Board*>(Native(world))->GetPosYBasedOnRow(posX, row);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_is_pool_square(const pvzp_rs_world* world, std::int32_t gridX, std::int32_t gridY, std::uint8_t* outIsPool) noexcept
{
	return Guard([&] {
		if (!world || !outIsPool)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outIsPool = static_cast<std::uint8_t>(const_cast<Board*>(Native(world))->IsPoolSquare(gridX, gridY));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_row_can_have_zombies(const pvzp_rs_world* world, std::int32_t row, std::uint8_t* outAllowed) noexcept
{
	return Guard([&] {
		if (!world || !outAllowed)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outAllowed = static_cast<std::uint8_t>(const_cast<Board*>(Native(world))->RowCanHaveZombies(row));
		return Ok();
	});
}
extern "C" pvzp_rs_status pvzp_rs_world_seed_count(const pvzp_rs_world* world, std::uint32_t* outCount) noexcept
{
	return Guard([&] {
		if (!world || !outCount)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		const auto* bank = Native(world)->mSeedBank.get();
		if (!bank)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		if (bank->mNumPackets < 0 || bank->mNumPackets > SEEDBANK_MAX)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, bank->mNumPackets);
		*outCount = static_cast<std::uint32_t>(bank->mNumPackets);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_seed_at(pvzp_rs_world* world, std::uint32_t index, pvzp_rs_seed** outSeed) noexcept
{
	return Guard([&] {
		if (!world || !outSeed)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* bank = Native(world)->mSeedBank.get();
		if (!bank)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		if (index >= static_cast<std::uint32_t>(bank->mNumPackets) || index >= SEEDBANK_MAX)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(index));
		*outSeed = &bank->mSeedPackets[index];
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_seed_can_pick_up(pvzp_rs_seed* seed, std::uint8_t* outCanPickUp) noexcept
{
	return Guard([&] {
		if (!seed || !outCanPickUp)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* cursor = seed->mBoard ? seed->mBoard->mCursorObject.get() : nullptr;
		const bool held = cursor && cursor->mCursorType == CursorType::CURSOR_TYPE_PLANT_FROM_BANK && cursor->mSeedBankIndex == seed->mIndex;
		*outCanPickUp = static_cast<std::uint8_t>(!held && seed->CanPickUp());
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_seed_was_planted(pvzp_rs_seed* seed) noexcept
{
	return Guard([&] {
		if (!seed)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		auto* board = seed->mBoard;
		if (!board || !board->mSeedBank) return Fail(PVZP_RS_STATUS_NOT_FOUND);
		auto* cursor = board->mCursorObject.get();
		const int held = cursor && cursor->mCursorType == CursorType::CURSOR_TYPE_PLANT_FROM_BANK ? cursor->mSeedBankIndex : -1;
		const int consumed = seed->mIndex;
		if (held == consumed) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, consumed);
		const bool conveyor = board->HasConveyorBeltSeedBank();
		seed->Deactivate();
		seed->WasPlanted();
		if (conveyor && held >= 0) {
			const int index = held > consumed ? held - 1 : held;
			if (index < board->mSeedBank->mNumPackets) {
				cursor->mSeedBankIndex = index;
				board->mSeedBank->mSeedPackets[index].Deactivate();
			}
		}
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_selected_card_count(std::uint32_t* outCount) noexcept
{
	return Guard([&] {
		if (!outCount)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		if (gLawnApp->mGameScene == GameScenes::SCENE_LEVEL_INTRO && gLawnApp->mSeedChooserScreen)
		{
			const int count = gLawnApp->mSeedChooserScreen->mSeedsInBank;
			if (count < 0 || count > SEEDBANK_MAX)
				return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, count);
			*outCount = static_cast<std::uint32_t>(count);
			return Ok();
		}
		if (gLawnApp->mGameScene == GameScenes::SCENE_PLAYING && gLawnApp->mBoard && gLawnApp->mBoard->mSeedBank)
		{
			const int count = gLawnApp->mBoard->mSeedBank->mNumPackets;
			if (count < 0 || count > SEEDBANK_MAX)
				return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, count);
			*outCount = static_cast<std::uint32_t>(count);
			return Ok();
		}
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	});
}

extern "C" pvzp_rs_status pvzp_rs_selected_card(std::uint32_t index, std::int32_t* outPacketType, std::int32_t* outImitaterType) noexcept
{
	return Guard([&] {
		if (!outPacketType || !outImitaterType)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		if (gLawnApp->mGameScene == GameScenes::SCENE_LEVEL_INTRO && gLawnApp->mSeedChooserScreen)
		{
			auto* chooser = gLawnApp->mSeedChooserScreen;
			if (index >= static_cast<std::uint32_t>(chooser->mSeedsInBank))
				return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(index));
			const SeedType packet = chooser->FindSeedInBank(static_cast<int>(index));
			if (packet < SeedType::SEED_PEASHOOTER || packet >= SeedType::NUM_SEED_TYPES)
				return Fail(PVZP_RS_STATUS_NOT_FOUND);
			const auto& chosen = chooser->mChosenSeeds[static_cast<std::size_t>(packet)];
			*outPacketType = static_cast<std::int32_t>(packet);
			*outImitaterType = static_cast<std::int32_t>(chosen.mImitaterType);
			return Ok();
		}
		if (gLawnApp->mGameScene == GameScenes::SCENE_PLAYING && gLawnApp->mBoard && gLawnApp->mBoard->mSeedBank)
		{
			auto* bank = gLawnApp->mBoard->mSeedBank.get();
			if (index >= static_cast<std::uint32_t>(bank->mNumPackets))
				return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(index));
			const auto& seed = bank->mSeedPackets[index];
			*outPacketType = static_cast<std::int32_t>(seed.mPacketType);
			*outImitaterType = static_cast<std::int32_t>(seed.mImitaterType);
			return Ok();
		}
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	});
}

extern "C" pvzp_rs_status pvzp_rs_select_card(std::int32_t packetType, std::int32_t imitaterType) noexcept
{
	return Guard([&] {
		if (!gLawnApp || gLawnApp->mGameScene != GameScenes::SCENE_LEVEL_INTRO || !gLawnApp->mSeedChooserScreen)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		if (!IsPacketType(packetType) || !IsImitaterTarget(imitaterType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, packetType);
		auto* chooser = gLawnApp->mSeedChooserScreen;
		const auto packet = static_cast<SeedType>(packetType);
		if (packet == SeedType::SEED_IMITATER)
		{
			if (imitaterType == static_cast<std::int32_t>(SeedType::SEED_NONE) || !chooser->mImitaterButton)
				return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, imitaterType);
			auto& imitater = chooser->mChosenSeeds[SeedType::SEED_IMITATER];
			imitater.mSeedState = ChosenSeedState::SEED_IN_CHOOSER;
			imitater.mImitaterType = static_cast<SeedType>(imitaterType);
			imitater.mX = chooser->mImitaterButton->mX;
			imitater.mY = chooser->mImitaterButton->mY;
			chooser->ClickedSeedInChooser(imitater);
			chooser->UpdateImitaterButton();
			return Ok();
		}
		if (imitaterType != static_cast<std::int32_t>(SeedType::SEED_NONE))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, imitaterType);
		chooser->ClickedSeedInChooser(chooser->mChosenSeeds[static_cast<std::size_t>(packet)]);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_current_plant_cost(const pvzp_rs_world* world, std::int32_t packetType, std::int32_t imitaterType, std::int32_t* outCost) noexcept
{
	return Guard([&] {
		if (!world || !outCost)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!IsPacketType(packetType) || !IsImitaterTarget(imitaterType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, packetType);
		*outCost = const_cast<Board*>(Native(world))->GetCurrentPlantCost(
			static_cast<SeedType>(packetType), static_cast<SeedType>(imitaterType));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_can_take_sun(const pvzp_rs_world* world, std::int32_t amount, std::uint8_t* outAllowed) noexcept
{
	return Guard([&] {
		if (!world || !outAllowed)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outAllowed = static_cast<std::uint8_t>(const_cast<Board*>(Native(world))->CanTakeSunMoney(amount));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_take_sun(pvzp_rs_world* world, std::int32_t amount, std::uint8_t* outTaken) noexcept
{
	return Guard([&] {
		if (!world || !outTaken)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outTaken = static_cast<std::uint8_t>(Native(world)->TakeSunMoney(amount));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_can_plant_at(const pvzp_rs_world* world, std::int32_t packetType, std::int32_t gridX, std::int32_t gridY, std::int32_t* outReason) noexcept
{
	return Guard([&] {
		if (!world || !outReason)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!IsPacketType(packetType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, packetType);
		*outReason = static_cast<std::int32_t>(const_cast<Board*>(Native(world))->CanPlantAt(
			gridX, gridY, static_cast<SeedType>(packetType)));
		return Ok();
	});
}

#define PVZP_RS_DEFINE_CREATE_PLANT(name, method) \
	extern "C" pvzp_rs_status pvzp_rs_world_##name(pvzp_rs_world* world, std::int32_t gridX, std::int32_t gridY, std::int32_t packetType, std::int32_t imitaterType, pvzp_rs_plant** outPlant) noexcept \
	{ \
		return Guard([&] { \
			if (!world || !outPlant) return Fail(PVZP_RS_STATUS_NULL_POINTER); \
			if (packetType < 0 || packetType > 52 || imitaterType < -1 || imitaterType > 52) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, packetType); \
			auto* board = Native(world); \
			const int rows = (board->mBackground == BackgroundType::BACKGROUND_3_POOL || board->mBackground == BackgroundType::BACKGROUND_4_FOG) ? 6 : 5; \
			if (gridX < 0 || gridX >= 9 || gridY < 0 || gridY >= rows || !board->mChallenge || board->mPlants.mSize >= board->mPlants.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT); \
			*outPlant = Native(world)->method(gridX, gridY, static_cast<SeedType>(packetType), static_cast<SeedType>(imitaterType)); \
			return Ok(); \
		}); \
	}

PVZP_RS_DEFINE_CREATE_PLANT(new_plant, NewPlant)
PVZP_RS_DEFINE_CREATE_PLANT(add_plant, AddPlant)

#undef PVZP_RS_DEFINE_CREATE_PLANT

extern "C" pvzp_rs_status pvzp_rs_plant_setup_izombie(pvzp_rs_plant* plant) noexcept {
    return Guard([&]{
        if(!plant||!plant->mBoard||!plant->mBoard->mChallenge)return Fail(PVZP_RS_STATUS_NULL_POINTER);
        plant->mBoard->mChallenge->IZombieSetupPlant(plant);return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_plant_die(pvzp_rs_plant* plant) noexcept
{
	return Guard([&] { if (!plant) return Fail(PVZP_RS_STATUS_NULL_POINTER); plant->Die(); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_plant_shovel(pvzp_rs_plant* plant) noexcept
{
	return Guard([&] {
		if (!plant || !plant->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (plant->NotOnGround()) return Ok();
		auto* board = plant->mBoard;
		plant->mApp->PlayFoley(FoleyType::FOLEY_USE_SHOVEL);
		++board->mPlantsShoveled;
		plant->Die();
		if (board->mTutorialState == TutorialState::TUTORIAL_SHOVEL_PICKUP)
			board->SetTutorialState(TutorialState::TUTORIAL_SHOVEL_DIG);
		if (board->mTutorialState == TutorialState::TUTORIAL_SHOVEL_DIG || board->mTutorialState == TutorialState::TUTORIAL_SHOVEL_KEEP_DIGGING)
			board->SetTutorialState(board->CountPlantByType(SeedType::SEED_PEASHOOTER) == 0 ? TutorialState::TUTORIAL_SHOVEL_COMPLETED : TutorialState::TUTORIAL_SHOVEL_KEEP_DIGGING);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_plant_imitater_morph(pvzp_rs_plant* plant, pvzp_rs_plant** outPlant) noexcept
{
    return Guard([&] {
        if (!plant || !outPlant || !plant->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        *outPlant = nullptr;
        if (plant->mDead || plant->mSeedType != SeedType::SEED_IMITATER) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        auto& pool = plant->mBoard->mPlants;
        if (pool.mSize >= pool.mMaxSize || pool.mFreeListHead >= pool.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        auto* successor = &pool.DataArrayGetItemAt(pool.mFreeListHead);
        plant->ImitaterMorph();
        *outPlant = successor;
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_plant_set_sleeping(pvzp_rs_plant* plant, std::uint8_t asleep) noexcept
{
	return Guard([&] { if (!plant) return Fail(PVZP_RS_STATUS_NULL_POINTER); plant->SetSleeping(asleep != 0); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_plant_play_idle(pvzp_rs_plant* plant, float fps) noexcept
{
	return Guard([&] { if (!plant) return Fail(PVZP_RS_STATUS_NULL_POINTER); plant->PlayIdleAnim(fps); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_plant_update_reanim_color(pvzp_rs_plant* plant) noexcept
{
	return Guard([&] { if (!plant) return Fail(PVZP_RS_STATUS_NULL_POINTER); plant->UpdateReanimColor(); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_plant_fire_cob(pvzp_rs_plant* plant, std::int32_t targetX, std::int32_t targetY) noexcept
{
	return Guard([&] {
		if (!plant)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (plant->mSeedType != SeedType::SEED_COBCANNON)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, static_cast<std::int32_t>(plant->mSeedType));
		if (!plant->mBoard || plant->NotOnGround() || plant->mState != PlantState::STATE_COBCANNON_READY)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
		auto* cursor = plant->mBoard->mCursorObject.get();
		if (cursor && cursor->mCursorType == CursorType::CURSOR_TYPE_COBCANNON_TARGET &&
			static_cast<std::uint32_t>(cursor->mCobCannonPlantID) == plant->mBoard->mPlants.DataArrayGetID(plant))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
		plant->CobCannonFire(targetX, targetY);
		return Ok();
	});
}

extern "C" void pvzp_rs_plant_hit_box(pvzp_rs_plant* plant, pvzp_rs_rect_i32* outRect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    CopyRect(plant->GetPlantRect(), outRect);
}

extern "C" void pvzp_rs_plant_attack_rect(pvzp_rs_plant* plant, std::int32_t weapon, pvzp_rs_rect_i32* outRect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    CopyRect(plant->GetPlantAttackRect(static_cast<PlantWeapon>(weapon)), outRect);
}

extern "C" std::uint32_t pvzp_rs_plant_damage_range_flags(pvzp_rs_plant* plant, std::int32_t weapon) noexcept
{
    // Caller provides a live plant and a validated primary/secondary weapon.
    return static_cast<std::uint32_t>(plant->GetDamageRangeFlags(static_cast<PlantWeapon>(weapon)));
}

extern "C" pvzp_rs_status pvzp_rs_plant_reanimation(pvzp_rs_plant* plant, pvzp_rs_reanimation** outReanimation) noexcept
{
	return Guard([&] {
		if (!plant || !outReanimation)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!gLawnApp)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
		*outReanimation = gLawnApp->ReanimationTryToGet(plant->mBodyReanimID);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_zombie_pos_y_based_on_row(pvzp_rs_zombie* zombie, std::int32_t row, float* outY) noexcept
{
	return Guard([&] {
		if (!zombie || !outY)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outY = zombie->GetPosYBasedOnRow(row);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_zombie_hit_box(pvzp_rs_zombie* zombie, pvzp_rs_rect_i32* outRect) noexcept
{
	return Guard([&] {
		if (!zombie || !outRect)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		CopyRect(zombie->GetZombieRect(), outRect);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_zombie_effected_by_damage(pvzp_rs_zombie* zombie, std::uint32_t flags, std::uint8_t* outEffected) noexcept
{
	return Guard([&] {
		if (!zombie || !outEffected)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		*outEffected = static_cast<std::uint8_t>(zombie->EffectedByDamage(flags));
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_zombie_can_target_plant(pvzp_rs_zombie* zombie, pvzp_rs_plant* plant, std::int32_t attackType, std::uint8_t* outCanTarget) noexcept
{
	return Guard([&] {
		if (!zombie || !plant || !outCanTarget)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (attackType < static_cast<std::int32_t>(ZombieAttackType::ATTACKTYPE_CHEW) || attackType > static_cast<std::int32_t>(ZombieAttackType::ATTACKTYPE_LADDER))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, attackType);
		*outCanTarget = static_cast<std::uint8_t>(zombie->CanTargetPlant(plant, static_cast<ZombieAttackType>(attackType)));
		return Ok();
	});
}

extern "C" std::uint8_t pvzp_rs_zombie_is_dead_or_dying(pvzp_rs_zombie* zombie) noexcept { return static_cast<std::uint8_t>(zombie->IsDeadOrDying()); }

extern "C" pvzp_rs_status pvzp_rs_zombie_reanimation(pvzp_rs_zombie* zombie, pvzp_rs_reanimation** outReanimation) noexcept
{
	return Guard([&] {
		if (!zombie || !outReanimation)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		// GameObject binds mApp at construction. The borrowed zombie keeps its
		// owning world alive; resolve only the optional child for this read.
		Reanimation* reanimation = zombie->mApp->ReanimationTryToGet(zombie->mBodyReanimID);
		*outReanimation = reanimation && !reanimation->mDead ? reanimation : nullptr;
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_add_zombie_in_row(pvzp_rs_world* world, std::int32_t zombieType, std::int32_t row, std::int32_t fromWave, pvzp_rs_zombie** outZombie) noexcept
{
	return Guard([&] {
		if (!world || !outZombie)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!IsZombieType(zombieType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, zombieType);
        auto* board=Native(world);
        const int rows=(board->mBackground==BackgroundType::BACKGROUND_3_POOL||board->mBackground==BackgroundType::BACKGROUND_4_FOG)?6:5;
        if(row<0||row>=rows)return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT,row);
        if(zombieType==ZombieType::ZOMBIE_BOBSLED && fromWave!=Zombie::ZOMBIE_WAVE_UI && fromWave!=Zombie::ZOMBIE_WAVE_CUTSCENE && board->mZombies.mMaxSize-board->mZombies.mSize<4) {
            *outZombie=nullptr;return Ok();
        }
		*outZombie = Native(world)->AddZombieInRow(static_cast<ZombieType>(zombieType), row, fromWave);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_world_place_zombie(pvzp_rs_world* world, std::int32_t zombieType, std::int32_t gridX, std::int32_t gridY, pvzp_rs_zombie** outZombie) noexcept
{
	return Guard([&] {
		if (!world || !outZombie)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!IsZombieType(zombieType))
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, zombieType);
		if (!Native(world)->mChallenge)
			return Fail(PVZP_RS_STATUS_NOT_FOUND);
        auto* board=Native(world);
        const int rows=(board->mBackground==BackgroundType::BACKGROUND_3_POOL||board->mBackground==BackgroundType::BACKGROUND_4_FOG)?6:5;
        const unsigned required=zombieType==ZombieType::ZOMBIE_BOBSLED?4:2;
        if(gridX<0||gridX>=9||gridY<0||gridY>=rows||board->mZombies.mMaxSize-board->mZombies.mSize<required)
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
		*outZombie = Native(world)->mChallenge->IZombiePlaceZombie(static_cast<ZombieType>(zombieType), gridX, gridY);
		return Ok();
	});
}

extern "C" pvzp_rs_status pvzp_rs_zombie_die_no_loot(pvzp_rs_zombie* zombie) noexcept
{
	return Guard([&] { if (!zombie) return Fail(PVZP_RS_STATUS_NULL_POINTER); zombie->DieNoLoot(); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_zombie_die_with_loot(pvzp_rs_zombie* zombie) noexcept
{
	return Guard([&] { if (!zombie) return Fail(PVZP_RS_STATUS_NULL_POINTER); zombie->DieWithLoot(); return Ok(); });
}

extern "C" void pvzp_rs_projectile_hit_box(pvzp_rs_projectile* projectile, pvzp_rs_rect_i32* outRect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    CopyRect(projectile->GetProjectileRect(), outRect);
}

#define PVZP_RS_DEFINE_CREATE_GRID_ITEM(name, method) \
	extern "C" pvzp_rs_status pvzp_rs_world_##name(pvzp_rs_world* world, std::int32_t gridX, std::int32_t gridY, pvzp_rs_grid_item** outItem) noexcept \
	{ \
		return Guard([&] { \
			if (!world || !outItem) return Fail(PVZP_RS_STATUS_NULL_POINTER); \
			*outItem = Native(world)->method(gridX, gridY); \
			return Ok(); \
		}); \
	}

PVZP_RS_DEFINE_CREATE_GRID_ITEM(add_ladder, AddALadder)
PVZP_RS_DEFINE_CREATE_GRID_ITEM(add_crater, AddACrater)
PVZP_RS_DEFINE_CREATE_GRID_ITEM(add_gravestone, AddAGraveStone)

extern "C" pvzp_rs_status pvzp_rs_world_process_delete_queue(pvzp_rs_world* world) noexcept {
    return Guard([&] {
        if (!world || !gLawnApp || Native(world)!=gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        // The caller excludes native traversal and outstanding entity borrows.
        // A pause/Continue dialog does not invalidate that outer-frame boundary.
        if (gLawnApp->mGameScene!=GameScenes::SCENE_PLAYING && gLawnApp->mGameScene!=GameScenes::SCENE_LEVEL_INTRO) return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        Native(world)->ProcessDeleteQueue();
        if(gLawnApp->mEffectSystem)gLawnApp->mEffectSystem->ProcessDeleteQueue();
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_spawn_challenge_grave(pvzp_rs_world* world,std::int32_t row,std::int32_t col) noexcept {
    return Guard([&] {
        if (!world || !Native(world)->mChallenge) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* board=Native(world);
        if (row<0 || row>=MAX_GRID_SIZE_Y || col<0 || col>=MAX_GRID_SIZE_X) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        if (board->mGridItems.mSize>=board->mGridItems.mMaxSize || board->mGridItems.mFreeListHead>=board->mGridItems.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        board->mChallenge->GraveDangerSpawnGraveAt(col,row);
        return Ok();
    });
}

extern "C" pvzp_rs_status pvzp_rs_world_place_rake(pvzp_rs_world* world,std::int32_t row,std::int32_t col,std::uint8_t* placed) noexcept {
    return Guard([&] {
        if (!world || !placed || !Native(world)->mApp->mPlayerInfo) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* board=Native(world);
        if (row<0 || row>=MAX_GRID_SIZE_Y || col<0 || col>=MAX_GRID_SIZE_X || board->mGridItems.mSize>=board->mGridItems.mMaxSize || board->mGridItems.mFreeListHead>=board->mGridItems.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        *placed=board->PlaceRakeAt(col,row)!=nullptr;
        return Ok();
    });
}

#undef PVZP_RS_DEFINE_CREATE_GRID_ITEM

extern "C" pvzp_rs_status pvzp_rs_grid_item_die(pvzp_rs_grid_item* item) noexcept
{
	return Guard([&] { if (!item) return Fail(PVZP_RS_STATUS_NULL_POINTER); item->GridItemDie(); return Ok(); });
}

extern "C" pvzp_rs_status pvzp_rs_item_collect(pvzp_rs_item* item, std::uint8_t playSound) noexcept
{
	return Guard([&] {
		if (!item)
			return Fail(PVZP_RS_STATUS_NULL_POINTER);
		if (!item->mBoard || item->mApp->mGameScene != GameScenes::SCENE_PLAYING)
			return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
		if (item->mDead || item->mIsBeingCollected || item->mBoard->mPaused || item->mBoard->mTimeStopCounter > 0)
			return Ok();
		if (playSound != 0)
			item->PlayCollectSound();
		item->Collect();
		return Ok();
	});
}

#define PVZP_RS_DEFINE_BOOL_RULE(name, member) \
    extern "C" std::uint8_t pvzp_rs_##name() noexcept \
    { return static_cast<std::uint8_t>(gModifiers.member); } \
	extern "C" pvzp_rs_status pvzp_rs_set_##name(std::uint8_t enabled) noexcept \
	{ \
		gModifiers.member = enabled != 0; return Ok(); \
	}

PVZP_RS_DEFINE_BOOL_RULE(seed_recharge_ignored, seedRechargeIgnored)
PVZP_RS_DEFINE_BOOL_RULE(upgrade_planting_unrestricted, upgradePlantingUnrestricted)
PVZP_RS_DEFINE_BOOL_RULE(seed_recharge_shading_hidden, seedRechargeShadingHidden)
PVZP_RS_DEFINE_BOOL_RULE(seed_bank_topmost, seedBankTopmost)
PVZP_RS_DEFINE_BOOL_RULE(sun_cost_ignored, sunCostIgnored)
PVZP_RS_DEFINE_BOOL_RULE(fog_revealed, fogRevealed)
PVZP_RS_DEFINE_BOOL_RULE(vase_contents_visible, vaseContentsVisible)
extern "C" std::int32_t pvzp_rs_plant_special_countdown_rule() noexcept { return gModifiers.plantSpecialCountdownRule; }
extern "C" pvzp_rs_status pvzp_rs_set_plant_special_countdown_rule(std::int32_t rule) noexcept
{
    if (rule < 0 || rule > 2) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gModifiers.plantSpecialCountdownRule = rule;
    return Ok();
}
extern "C" std::uint8_t pvzp_rs_instant_ice_and_ash_effects() noexcept { return gModifiers.plantSpecialCountdownRule == 1; }
extern "C" pvzp_rs_status pvzp_rs_set_instant_ice_and_ash_effects(std::uint8_t enabled) noexcept
{
    return pvzp_rs_set_plant_special_countdown_rule(enabled ? 1 : 0);
}
PVZP_RS_DEFINE_BOOL_RULE(mushrooms_awake, mushroomsAwake)
PVZP_RS_DEFINE_BOOL_RULE(cob_fixed_delay, cobFixedDelay)
PVZP_RS_DEFINE_BOOL_RULE(cob_recharge_shortened, cobRechargeShortened)
PVZP_RS_DEFINE_BOOL_RULE(cob_drift_fixed, cobDriftFixed)
PVZP_RS_DEFINE_BOOL_RULE(item_drop_disabled, itemDropDisabled)
PVZP_RS_DEFINE_BOOL_RULE(natural_sun_drop_disabled, naturalSunDropDisabled)
extern "C" std::int32_t pvzp_rs_jack_explosion_rule() noexcept { return gModifiers.jackExplosionRule; }
extern "C" pvzp_rs_status pvzp_rs_set_jack_explosion_rule(std::int32_t rule) noexcept
{
    if (rule < 0 || rule > 2) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gModifiers.jackExplosionRule = rule;
    return Ok();
}
extern "C" std::uint8_t pvzp_rs_jack_explosions_disabled() noexcept { return gModifiers.jackExplosionRule == 2; }
extern "C" pvzp_rs_status pvzp_rs_set_jack_explosions_disabled(std::uint8_t enabled) noexcept
{ return pvzp_rs_set_jack_explosion_rule(enabled ? 2 : 0); }
extern "C" std::int32_t pvzp_rs_pepper_explosion_rule() noexcept { return gModifiers.pepperExplosionRule; }
extern "C" pvzp_rs_status pvzp_rs_set_pepper_explosion_rule(std::int32_t rule) noexcept
{
    if (rule < 0 || rule > 2) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gModifiers.pepperExplosionRule = rule;
    return Ok();
}
extern "C" std::uint8_t pvzp_rs_pepper_explosions_disabled() noexcept { return gModifiers.pepperExplosionRule == 2; }
extern "C" pvzp_rs_status pvzp_rs_set_pepper_explosions_disabled(std::uint8_t enabled) noexcept
{ return pvzp_rs_set_pepper_explosion_rule(enabled ? 2 : 0); }
PVZP_RS_DEFINE_BOOL_RULE(special_events_disabled, specialEventsDisabled)
PVZP_RS_DEFINE_BOOL_RULE(zombie_spawn_stopped, zombieSpawnStopped)
PVZP_RS_DEFINE_BOOL_RULE(zombies_die_at_house, zombiesDieAtHouse)
PVZP_RS_DEFINE_BOOL_RULE(planting_restrictions_ignored, plantingRestrictionsIgnored)
PVZP_RS_DEFINE_BOOL_RULE(profile_readonly, profileReadonly)
PVZP_RS_DEFINE_BOOL_RULE(craters_expire_immediately, cratersExpireImmediately)
PVZP_RS_DEFINE_BOOL_RULE(chilled_effects_disabled, chilledEffectsDisabled)
PVZP_RS_DEFINE_BOOL_RULE(butter_effects_disabled, butterEffectsDisabled)
PVZP_RS_DEFINE_BOOL_RULE(imp_throw_disabled, impThrowDisabled)
PVZP_RS_DEFINE_BOOL_RULE(zombie_walking_stopped, zombieWalkingStopped)
PVZP_RS_DEFINE_BOOL_RULE(ice_trails_disabled, iceTrailsDisabled)
PVZP_RS_DEFINE_BOOL_RULE(pause_dialog_disabled, pauseDialogDisabled)
PVZP_RS_DEFINE_BOOL_RULE(hidden_pages_visible, hiddenPagesVisible)
extern "C" std::int32_t pvzp_rs_background_run_rule() noexcept { return gModifiers.backgroundRunRule; }
extern "C" pvzp_rs_status pvzp_rs_set_background_run_rule(std::int32_t rule) noexcept
{
    if (rule < 0 || rule > 2) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gModifiers.backgroundRunRule = rule;
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_set_ice_trail_countdown(std::uint8_t row, std::int32_t value) noexcept
{
    if (!gLawnApp || !gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (row >= MAX_GRID_SIZE_Y || value < 0) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    gLawnApp->mBoard->mIceTimer[row] = value;
    return Ok();
}
PVZP_RS_DEFINE_BOOL_RULE(normal_auto_collect_enabled, normalAutoCollectEnabled)

#undef PVZP_RS_DEFINE_BOOL_RULE

extern "C" pvzp_rs_status pvzp_rs_easy_planting_cheat(std::uint8_t* outEnabled) noexcept
{
	if (!outEnabled)
		return Fail(PVZP_RS_STATUS_NULL_POINTER);
	if (!gLawnApp)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	*outEnabled = static_cast<std::uint8_t>(gLawnApp->mEasyPlantingCheat);
	return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_set_easy_planting_cheat(std::uint8_t enabled) noexcept
{
	if (!gLawnApp)
		return Fail(PVZP_RS_STATUS_NOT_FOUND);
	if (!gEasyPlantingCaptured)
	{
		gEasyPlantingOriginal = gLawnApp->mEasyPlantingCheat;
		gEasyPlantingCaptured = true;
	}
	gLawnApp->mEasyPlantingCheat = enabled != 0;
	return Ok();
}

#define PVZP_RS_DEFINE_ENUM_RULE(name, member, maximum) \
	extern "C" pvzp_rs_status pvzp_rs_##name(std::int32_t* outRule) noexcept \
	{ \
		if (!outRule) return Fail(PVZP_RS_STATUS_NULL_POINTER); *outRule = gModifiers.member; return Ok(); \
	} \
	extern "C" pvzp_rs_status pvzp_rs_set_##name(std::int32_t rule) noexcept \
	{ \
		if (rule < 0 || rule > maximum) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, rule); gModifiers.member = rule; return Ok(); \
	}

PVZP_RS_DEFINE_ENUM_RULE(kernel_pult_projectile_rule, kernelPultProjectileRule, 2)
PVZP_RS_DEFINE_ENUM_RULE(plant_damage_rule, plantDamageRule, 2)
PVZP_RS_DEFINE_ENUM_RULE(zombie_damage_rule, zombieDamageRule, 2)
PVZP_RS_DEFINE_ENUM_RULE(maid_cheat, maidCheat, 3)

#undef PVZP_RS_DEFINE_ENUM_RULE

#define PVZP_RS_DEFINE_WORLD_POOL(name, member, pool_type) \
    extern "C" pool_type* pvzp_rs_world_##name##_pool(pvzp_rs_world* world) noexcept \
    { return reinterpret_cast<pool_type*>(&Native(world)->member); }


PVZP_RS_DEFINE_WORLD_POOL(plant, mPlants, pvzp_rs_plant_pool)
PVZP_RS_DEFINE_WORLD_POOL(zombie, mZombies, pvzp_rs_zombie_pool)
PVZP_RS_DEFINE_WORLD_POOL(projectile, mProjectiles, pvzp_rs_projectile_pool)
PVZP_RS_DEFINE_WORLD_POOL(grid_item, mGridItems, pvzp_rs_grid_item_pool)
PVZP_RS_DEFINE_WORLD_POOL(item, mCoins, pvzp_rs_item_pool)
PVZP_RS_DEFINE_WORLD_POOL(mower, mLawnMowers, pvzp_rs_mower_pool)

#undef PVZP_RS_DEFINE_WORLD_POOL

#define PVZP_RS_DEFINE_POOL_API(prefix, pool_type, object_type) \
	extern "C" std::uint32_t pvzp_rs_##prefix##_pool_max_used_count(const pool_type* pool) noexcept \
    { return NativePool<object_type>(pool)->mMaxUsedCount; } \
	extern "C" std::uint32_t pvzp_rs_##prefix##_pool_active_count(const pool_type* pool) noexcept \
    { return NativePool<object_type>(pool)->mSize; } \
	extern "C" std::uint32_t pvzp_rs_##prefix##_pool_capacity(const pool_type* pool) noexcept \
    { return NativePool<object_type>(pool)->mMaxSize; } \
	extern "C" std::uint32_t pvzp_rs_##prefix##_pool_free_list_head(const pool_type* pool) noexcept \
    { return NativePool<object_type>(pool)->mFreeListHead; } \
	extern "C" std::uint32_t pvzp_rs_##prefix##_pool_next_id_key(const pool_type* pool) noexcept \
    { return NativePool<object_type>(pool)->mNextKey; } \
	extern "C" pvzp_rs_status pvzp_rs_##prefix##_pool_slot(const pool_type* pool, std::uint32_t index, std::uint8_t* outActive, std::uint32_t* outId, std::uint32_t* outNextFree) noexcept \
	{ \
		return Guard([&] { return PoolSlot<object_type>(pool, index, outActive, outId, outNextFree); }); \
	} \
	extern "C" pvzp_rs_status pvzp_rs_##prefix##_pool_get(pool_type* pool, std::int32_t index, object_type** outObject) noexcept \
	{ \
		return Guard([&] { return PoolGet<object_type>(pool, index, outObject); }); \
	} \
	extern "C" object_type* pvzp_rs_##prefix##_pool_try_to_get(pool_type* pool, std::uint32_t id) noexcept \
    { return NativePool<object_type>(pool)->DataArrayTryToGet(id); } \
	extern "C" pvzp_rs_status pvzp_rs_##prefix##_pool_get_id(const pool_type* pool, const object_type* object, std::uint32_t* outId) noexcept \
	{ \
		return Guard([&] { return PoolGetId<object_type>(pool, object, outId); }); \
	}

PVZP_RS_DEFINE_POOL_API(plant, pvzp_rs_plant_pool, pvzp_rs_plant)
PVZP_RS_DEFINE_POOL_API(zombie, pvzp_rs_zombie_pool, pvzp_rs_zombie)
PVZP_RS_DEFINE_POOL_API(projectile, pvzp_rs_projectile_pool, pvzp_rs_projectile)
PVZP_RS_DEFINE_POOL_API(grid_item, pvzp_rs_grid_item_pool, pvzp_rs_grid_item)
PVZP_RS_DEFINE_POOL_API(item, pvzp_rs_item_pool, pvzp_rs_item)
PVZP_RS_DEFINE_POOL_API(mower, pvzp_rs_mower_pool, pvzp_rs_mower)

#undef PVZP_RS_DEFINE_POOL_API
extern "C" std::int32_t pvzp_rs_seed_base_cost(std::int32_t kind) noexcept {
    return kind >= 0 && kind < 48 ? GetPlantDefinition(static_cast<SeedType>(kind)).mSeedCost : -1;
}

extern "C" std::int32_t pvzp_rs_seed_base_recharge(std::int32_t kind) noexcept {
    return kind >= 0 && kind < 48 ? GetPlantDefinition(static_cast<SeedType>(kind)).mRefreshTime : -1;
}

// Plant has only scalar state and native resource IDs. Its constructor is empty
// apart from GameObject defaults and its destructor owns no resources; Die is a
// separate gameplay action. RenderItem/PlantsOnLawn/HitResult pointers are local
// to native calls, so relocation is restricted to an outer ready update boundary.
extern "C" pvzp_rs_status pvzp_rs_world_plant_slot_id(pvzp_rs_world* world, std::uint32_t slot, std::uint32_t* id) noexcept
{
    if (!world || !id) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    auto& pool = Native(world)->mPlants;
    if (slot >= pool.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    *id = slot < pool.mMaxUsedCount && (pool.DataArrayGetIDAt(slot) & DATA_ARRAY_KEY_MASK) ? pool.DataArrayGetIDAt(slot) : 0;
    return Ok();
}

extern "C" pvzp_rs_status pvzp_rs_world_relocate_plant_slot(pvzp_rs_world* world, std::uint32_t sourceId, std::uint32_t target,
    std::uint32_t expectedTarget, std::uint32_t* moved, std::uint32_t* swapped, std::uint8_t commit) noexcept
{
    return Guard([&] {
        static_assert(std::is_trivially_copyable_v<Plant> && std::is_trivially_destructible_v<Plant>);
        if (!world || !moved || !swapped || !gLawnApp || Native(world) != gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        Board* board = Native(world);
        if (board->mPaused || (gLawnApp->mWidgetManager && gLawnApp->mWidgetManager->mBaseModalWidget)
            || (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING && gLawnApp->mGameScene != GameScenes::SCENE_LEVEL_INTRO))
            return Fail(PVZP_RS_STATUS_UNSUPPORTED);
        auto& pool = board->mPlants;
        if (!pool.mMaxSize || pool.mMaxSize >= DATA_ARRAY_MAX_SIZE || pool.mMaxUsedCount > pool.mMaxSize
            || pool.mSize > pool.mMaxUsedCount || pool.mFreeListHead > pool.mMaxUsedCount || !pool.mNextKey || pool.mNextKey >= DATA_ARRAY_MAX_SIZE)
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 1);
        Plant* source = pool.DataArrayTryToGet(sourceId);
        if (!source || source->mDead || target >= pool.mMaxSize) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 2);
        const unsigned sourceSlot = sourceId & DATA_ARRAY_INDEX_MASK;
        const unsigned targetId = target < pool.mMaxUsedCount && (pool.DataArrayGetIDAt(target) & DATA_ARRAY_KEY_MASK) ? pool.DataArrayGetIDAt(target) : 0;
        if (sourceSlot == target) {
            if (expectedTarget && expectedTarget != sourceId) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 3);
            *moved = sourceId; *swapped = 0; return Ok();
        }
        if (targetId != expectedTarget) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 3);
        const unsigned oldMax = pool.mMaxUsedCount;
        unsigned previous = oldMax, targetPrevious = oldMax, tailPrevious = oldMax, freeCount = 0;
        bool foundTarget = false;
        if (!targetId) {
            unsigned slot = pool.mFreeListHead;
            while (slot != oldMax) {
                if (slot >= oldMax || (pool.DataArrayGetIDAt(slot) & DATA_ARRAY_KEY_MASK) || ++freeCount > oldMax)
                    return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 4);
                if (slot == target) { foundTarget = true; targetPrevious = previous; }
                previous = slot;
                slot = pool.DataArrayGetIDAt(slot);
            }
            tailPrevious = previous;
            if (freeCount != oldMax - pool.mSize || (target < oldMax && !foundTarget))
                return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT, 4);
        }
        if (!commit) { *moved=sourceId; *swapped=targetId; return Ok(); }
        auto freshId = [&](unsigned slot, unsigned oldId) {
            unsigned value;
            do {
                value = (pool.mNextKey++ << DATA_ARRAY_KEY_SHIFT) | slot;
                if (pool.mNextKey == DATA_ARRAY_MAX_SIZE) pool.mNextKey = 1;
            } while (value == oldId);
            return value;
        };
        const unsigned newSource = freshId(target, targetId);
        const unsigned newOther = targetId ? freshId(sourceSlot, sourceId) : 0;
        Plant* destination = &pool.DataArrayGetItemAt(target);
        if (targetId) {
            // No constructor, Die, animation reset or resource allocation.
            alignas(Plant) unsigned char temporary[sizeof(Plant)];
            std::memcpy(temporary, source, sizeof(Plant));
            std::memcpy(source, destination, sizeof(Plant));
            std::memcpy(destination, temporary, sizeof(Plant));
            pool.DataArrayGetIDAt(sourceSlot) = newOther;
        } else {
            if (target < oldMax) {
                const unsigned next = pool.DataArrayGetIDAt(target);
                if (targetPrevious == oldMax) pool.mFreeListHead = next;
                else pool.DataArrayGetIDAt(targetPrevious) = next;
            } else {
                if (target == oldMax) {
                    if (tailPrevious == oldMax) pool.mFreeListHead = target + 1;
                    else pool.DataArrayGetIDAt(tailPrevious) = target + 1;
                } else {
                    for (unsigned slot = oldMax; slot < target; ++slot) pool.DataArrayGetIDAt(slot) = slot + 1;
                    pool.DataArrayGetIDAt(target - 1) = target + 1;
                }
                pool.mMaxUsedCount = target + 1;
            }
            std::memcpy(destination, source, sizeof(Plant));
            // The now-free payload is ignored and is value-initialized on next
            // allocation. Its trivial destructor cannot destroy copied resources.
            pool.DataArrayGetIDAt(sourceSlot) = pool.mFreeListHead;
            pool.mFreeListHead = sourceSlot;
        }
        pool.DataArrayGetIDAt(target) = newSource;
        auto remap = [&](PlantID& id) {
            if (static_cast<unsigned>(id) == sourceId) id = static_cast<PlantID>(newSource);
            else if (targetId && static_cast<unsigned>(id) == targetId) id = static_cast<PlantID>(newOther);
        };
        for (auto* zombie : board->mZombies) remap(zombie->mTargetPlantID);
        if (board->mCursorObject) {
            remap(board->mCursorObject->mGlovePlantID);
            remap(board->mCursorObject->mDuplicatorPlantID);
            remap(board->mCursorObject->mCobCannonPlantID);
        }
        gMorphPlaceholder = gMorphSuccessor = 0;
        *moved = newSource; *swapped = newOther;
        return Ok();
    });
}

extern "C" std::uint32_t pvzp_rs_mower_id(const pvzp_rs_mower* mower) noexcept
{
    return mower->mBoard->mLawnMowers.DataArrayGetID(const_cast<LawnMower*>(mower));
}
extern "C" pvzp_rs_status pvzp_rs_mower_start(pvzp_rs_mower* mower) noexcept
{
    return Guard([&] {
        if (!mower) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (!mower->mDead) mower->StartMower();
        return Ok();
    });
}
extern "C" pvzp_rs_status pvzp_rs_mower_die(pvzp_rs_mower* mower) noexcept
{
    return Guard([&] {
        if (!mower) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        if (mower->mDead) return Ok();
        auto* board = mower->mBoard;
        // Same admission as native LawnMower::Die / Board::HasLevelAwardDropped.
        const bool award = board->mLevelAwardSpawned || board->mNextSurvivalStageCounter > 0 || board->mBoardFadeOutCounter >= 0;
        if (board->mBonusLawnMowersRemaining > 0 && !award && board->mLawnMowers.mSize >= board->mLawnMowers.mMaxSize)
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        mower->Die();
        return Ok();
    });
}
extern "C" pvzp_rs_status pvzp_rs_world_initialize_mowers_ready(pvzp_rs_world* world) noexcept
{
    return Guard([&] {
        if (!world) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* board = Native(world);
        unsigned required = 0;
        for (int row = 0; row < 6; ++row)
            if ((board->mApp->mGameMode == GameMode::GAMEMODE_CHALLENGE_RESODDED && row < 5) || board->mPlantRow[row] != PlantRowType::PLANTROW_DIRT) ++required;
        if (!board->mApp->mPlayerInfo || board->mLawnMowers.mMaxSize - board->mLawnMowers.mSize < required)
            return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
        board->InitLawnMowersReady();
        return Ok();
    });
}

std::uint8_t pvzp_rs_set_paint(void (*callback)()) noexcept {
    try { return PvzpPlugin::SetPaintCallback(callback); } catch (...) { return 0; }
}
std::uint8_t pvzp_rs_paint_size(std::uint32_t* width, std::uint32_t* height) noexcept {
    return PvzpPlugin::PaintSize(width,height);
}
std::uint8_t pvzp_rs_paint_rect(int x,int y,int width,int height,std::uint32_t argb) noexcept {
    try { return PvzpPlugin::PaintRect(x,y,width,height,argb); } catch (...) { return 0; }
}
std::uint8_t pvzp_rs_paint_clip(std::uint8_t enabled,int x,int y,int width,int height) noexcept {
    return PvzpPlugin::PaintClip(enabled != 0,x,y,width,height);
}
std::uint8_t pvzp_rs_paint_text(int x,int y,const char* text,int length,const std::uint16_t* family,
    int pixels,std::uint8_t bold,std::uint32_t argb,std::uint8_t measure,std::uint32_t* width,std::uint32_t* height) noexcept {
    return PvzpPlugin::PaintText(x,y,text,length,reinterpret_cast<const char16_t*>(family),pixels,bold != 0,argb,measure != 0,width,height);
}
std::uint8_t pvzp_rs_paint_digit_bounds(const std::uint16_t* family, int pixels, std::uint8_t bold,
    int* top, int* bottom) noexcept {
    try { return PvzpPlugin::PaintDigitBounds(reinterpret_cast<const char16_t*>(family), pixels, bold != 0, top, bottom); }
    catch (...) { return 0; }
}

extern "C" pvzp_rs_status pvzp_rs_world_zombie_refresh_health_threshold(const pvzp_rs_world* world, std::int32_t* out) noexcept {
    if (!world || !out) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    *out = Native(world)->mZombieHealthToNextWave; return Ok();
}

std::uint8_t pvzp_rs_paint_pointer(std::uint8_t sprite,int* x,int* y) noexcept {
    return PvzpPlugin::PaintPointer(sprite!=0,x,y);
}

std::uint8_t pvzp_rs_paint_grid_row(int* row) noexcept {return PvzpPlugin::PaintGridRow(row);}

extern "C" pvzp_rs_status pvzp_rs_hud_read(std::uint32_t field,std::int32_t* value) noexcept {
    if (!gLawnApp || !gLawnApp->mBoard || !value) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    auto* board=gLawnApp->mBoard;
    switch(field) {
        case 0:*value=board->mShowShovel;break;
        case 1:*value=board->mCoinBankFadeCount;break;
        case 2:if(!board->mMenuButton)return Fail(PVZP_RS_STATUS_NULL_POINTER);*value=board->mMenuButton->mBtnNoDraw;break;
        default:return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    }
    return Ok();
}
extern "C" pvzp_rs_status pvzp_rs_hud_write(std::uint32_t field,std::int32_t value) noexcept {
    if (!gLawnApp || !gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
    if (field!=1 && value!=0 && value!=1) return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    auto* board=gLawnApp->mBoard;
    switch(field) {
        case 0:board->mShowShovel=value!=0;break;
        case 1:board->mCoinBankFadeCount=value;break;
        case 2:if(!board->mMenuButton)return Fail(PVZP_RS_STATUS_NULL_POINTER);board->mMenuButton->mBtnNoDraw=value!=0;break;
        default:return Fail(PVZP_RS_STATUS_INVALID_ARGUMENT);
    }
    board->MarkDirty();
    return Ok();
}

std::int32_t pvzp_rs_zombie_hurt_width(const pvzp_rs_zombie* zombie) noexcept {return zombie->mZombieRect.mWidth;}
std::int32_t pvzp_rs_zombie_hurt_height(const pvzp_rs_zombie* zombie) noexcept {return zombie->mZombieRect.mHeight;}

std::uint8_t pvzp_rs_paint_cursor(int x,int y,std::uint8_t hand) noexcept {return PvzpPlugin::PaintCursor(x,y,hand!=0);}
#include "PvzpLib/Replay.h"
extern "C" std::uint8_t pvzp_rs_replay_save(std::uintptr_t file) noexcept { return PvzpReplay::SaveCheckpoint(file); }
extern "C" std::uint8_t pvzp_rs_replay_capture(void* context, std::uint8_t (*receive)(void*, const unsigned char*, std::uint64_t)) noexcept { return PvzpReplay::CaptureCheckpoint(context, receive); }
extern "C" std::uint8_t pvzp_rs_replay_restore_bytes(const unsigned char* bytes, std::size_t length, std::int32_t mode) noexcept { return PvzpReplay::RestoreCheckpointBytes(bytes,length,mode); }
extern "C" std::uint8_t pvzp_rs_replay_restore(const char* path, std::int32_t mode) noexcept { return PvzpReplay::RestoreCheckpoint(path,mode); }
extern "C" std::uint8_t pvzp_rs_replay_mouse_x(std::int32_t* value) noexcept {if(!gLawnApp||!gLawnApp->mWidgetManager||!value)return 0;*value=gLawnApp->mWidgetManager->mLastMouseX;return 1;}
extern "C" std::uint8_t pvzp_rs_replay_mouse_y(std::int32_t* value) noexcept {if(!gLawnApp||!gLawnApp->mWidgetManager||!value)return 0;*value=gLawnApp->mWidgetManager->mLastMouseY;return 1;}
extern "C" std::uint8_t pvzp_rs_replay_mouse_buttons(std::int32_t* value) noexcept {if(!gLawnApp||!gLawnApp->mWidgetManager||!value)return 0;*value=gLawnApp->mWidgetManager->mDownButtons;return 1;}
extern "C" std::uint8_t pvzp_rs_replay_cursor_kind(std::int32_t* value) noexcept {if(!gLawnApp||!value)return 0;*value=gLawnApp->mCursorNum;return 1;}
extern "C" std::uint8_t pvzp_rs_replay_refresh() noexcept {
    if (!gLawnApp || !gLawnApp->mBoard) return 0;
    auto& countdown=gLawnApp->mBoard->mZombieCountDown;
    countdown=static_cast<int>(static_cast<std::uint32_t>(countdown)-1); return 1;
}
#include "PvzpLib/WorldEdits.h"
extern "C" std::uint8_t pvzp_rs_clear_projectiles() noexcept {return PvzpWorldEdits::ClearProjectiles();}
extern "C" std::uint8_t pvzp_rs_clear_items() noexcept {return PvzpWorldEdits::ClearItems();}
#include "user_card.inc"

extern "C" std::int32_t pvzp_rs_replay_emitter_count(std::int32_t kind) noexcept {return PvzpReplay::ParticleEmitterCount(kind);}
extern "C" std::int32_t pvzp_rs_replay_track_count(std::int32_t kind) noexcept {return PvzpReplay::AnimationTrackCount(kind);}
extern "C" std::int32_t pvzp_rs_replay_frame_count(std::int32_t kind) noexcept {return PvzpReplay::AnimationFrameCount(kind);}

extern "C" std::uint32_t pvzp_rs_paint_create_image(const std::uint32_t* pixels, int width, int height) noexcept { return PvzpPlugin::PaintCreateImage(pixels,width,height); }
extern "C" std::uint8_t pvzp_rs_paint_image(std::uint32_t id, int x, int y, std::uint32_t argb) noexcept { return PvzpPlugin::PaintImage(id,x,y,argb); }

extern "C" std::uint8_t pvzp_rs_input_begin(std::uint8_t playback) noexcept { return PvzpReplay::BeginInput(playback != 0); }
extern "C" std::uint8_t pvzp_rs_input_drain(Sexy::PointerInput* output, std::size_t capacity, std::size_t* count) noexcept { return PvzpReplay::DrainInput(output, capacity, count); }
extern "C" std::uint8_t pvzp_rs_input_apply(const Sexy::PointerInput* input) noexcept { return input && PvzpReplay::ApplyInput(*input); }
extern "C" void pvzp_rs_input_end() noexcept { PvzpReplay::EndInput(); }
extern "C" void pvzp_rs_input_hand_back() noexcept { PvzpReplay::HandBackInput(); }
extern "C" std::uint64_t pvzp_rs_input_sequence() noexcept { return PvzpReplay::InputSequence(); }

extern "C" std::int32_t pvzp_rs_input_terminal_clock() noexcept { return PvzpReplay::InputTerminalClock(); }

extern "C" std::uint8_t pvzp_rs_checkpoint_ready() noexcept { return PvzpReplay::CanSaveCheckpoint(); }

extern "C" void pvzp_rs_set_key_sink(void (*callback)(std::uint32_t)) noexcept { PvzpNative::SetKeySink(callback); }

extern "C" pvzp_rs_status pvzp_rs_clear_ice_trails() noexcept {
    return Guard([&] {
        if (!gLawnApp || !gLawnApp->mBoard) return Fail(PVZP_RS_STATUS_NULL_POINTER);
        auto* board = gLawnApp->mBoard;
        for (int row = 0; row < MAX_GRID_SIZE_Y; ++row) {
            auto* ice = gLawnApp->ParticleTryToGet(board->mIceParticleID[row]);
            board->mIceTimer[row] = 0;
            board->mIceMinX[row] = BOARD_ICE_START;
            if (ice) ice->ParticleSystemDie();
            board->mIceParticleID[row] = ParticleSystemID::PARTICLESYSTEMID_NULL;
        }
        return Ok();
    });
}
extern "C" void pvzp_rs_paint_source(std::uint32_t source) noexcept { PvzpPlugin::SetPaintSource(source); }
extern "C" void pvzp_rs_paint_clear_source(std::uint32_t source) noexcept { PvzpPlugin::ClearPaintSource(source); }


extern "C" std::int32_t pvzp_rs_diagnostic_dialog(const char* text) noexcept {
    return PvzpNative::DiagnosticDialog(text);
}
extern "C" void pvzp_rs_diagnostic_exit() noexcept {
    PvzpNative::DiagnosticExit();
}
