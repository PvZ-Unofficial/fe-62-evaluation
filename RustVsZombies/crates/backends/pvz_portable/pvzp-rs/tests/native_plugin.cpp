#include "bridge.h"
#include "PvzpLib/PluginLayout.h"
#include "PvzpLib/EffectSystem.h"
#include "PvzpLib/PvzpParticle.h"
#include "Lawn/Widget/GameSelector.h"
#include "SexyAppFramework/sound/SoundManager.h"
#include "SexyAppFramework/sound/SoundInstance.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdexcept>
#include <cstring>
#include <cstdio>
#include <algorithm>

static void Record(const char* text)
{
    wchar_t path[32768];
    if (!GetEnvironmentVariableW(L"PVZP_TEST_REPORT", path, 32768)) return;
    HANDLE file = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
        nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) return;
    DWORD written;
    WriteFile(file, text, static_cast<DWORD>(std::strlen(text)), &written, nullptr);
    WriteFile(file, "\n", 1, &written, nullptr);
    CloseHandle(file);
}
static void Check(bool value, const char* message) { if (!value) throw std::runtime_error(message); }
static void Ok(pvzp_rs_status result) { Check(result.code == 0, "native operation failed"); }
static int phase = 0;
static int dispatchCount = 0;
static std::uint64_t observedRounds = 0;
static int frameBegins = 0;
static int frameEnds = 0;
static int destroyedBoards = 0;
static bool destroyWasRevoked = true;
static std::uint64_t eventEpoch = 0;
static std::uint64_t firstEpoch = 0;
static std::uint32_t bitePlant = 0;
static std::uint32_t biter = 0;
static int biteHealth = 0;
static int biteAttempts = 0;
static int biteFinished = 0;
static int biteRequested = 0;
static int biteOutcome = -1;
static int biteApplied = 0;
static bool suppressBite = true;
static int pausedAppClock = 0;
static int pausedBoardClock = 0;
static void BeginFrame(std::uint64_t epoch, std::int32_t) { ++frameBegins; eventEpoch = epoch; }
static void EndFrame(std::int32_t) { ++frameEnds; }
static std::uint64_t BeginEffect(std::int32_t source, void*, void*, std::int32_t, std::int32_t requested)
{
    if (source != 1) return 0;
    ++biteAttempts;
    biteRequested = requested;
    return 1ULL | (std::uint64_t(suppressBite) << 32);
}
static void FinishEffect(std::uint32_t, std::int32_t outcome, std::int32_t applied)
{
    ++biteFinished;
    biteOutcome = outcome;
    biteApplied = applied;
}
static void ZombieEvent(void*) {}
static void ImpEvent(void*, void*) {}
static void Destroyed()
{
    ++destroyedBoards;
    destroyWasRevoked = destroyWasRevoked && gLawnApp->mPlugin.battleInterest == 0;
}
static void RegisterBattle(std::uint32_t interest = 1)
{
    const PvzpPlugin::BattleCallbacks callbacks{BeginFrame, EndFrame, BeginEffect, FinishEffect,
        ZombieEvent, ZombieEvent, ImpEvent, ZombieEvent};
    Check(PvzpPlugin::SetBattleCallbacks(&callbacks, interest), "battle registration failed");
}
static void Step()
{
    const auto before = dispatchCount;
    gLawnApp->UpdateFrames();
    Check(dispatchCount == before, "nested update dispatched another plugin callback");
}

static void MenuChecks()
{
    Check(!gLawnApp->mBoard, "menu test requires no Board");
    const auto frameTime = gLawnApp->mFrameTime;
    const auto multiplier = gLawnApp->mUpdateMultiplier;
    const auto clock = gLawnApp->mAppCounter;
    const auto planting = gLawnApp->mEasyPlantingCheat;
    // A non-default frame interval must survive both speed changes and restore.
    gLawnApp->mFrameTime = 17;
    for (float speed : {0.05f, 0.25f, 1.5f, 3.0f, 10.0f, 20.0f, 100.0f})
    {
        Ok(pvzp_rs_set_game_speed(speed));
        Check(gLawnApp->mFrameTime == 17 && gLawnApp->mUpdateMultiplier == speed,
            "speed must only change the update multiplier");
    }
    Check(pvzp_rs_set_game_speed(0).code != 0 && pvzp_rs_set_game_speed(101).code != 0,
        "invalid game speed accepted");
    gLawnApp->mFrameTime = 19;
    Ok(pvzp_rs_restore_game_speed());
    Check(gLawnApp->mFrameTime == 19 && gLawnApp->mUpdateMultiplier == multiplier,
        "restore must not overwrite frame interval changes");
    gLawnApp->mFrameTime = frameTime;
    const double exactMultiplier = 0.123456789012345;
    Ok(pvzp_rs_set_game_speed_multiplier(exactMultiplier));
    double actualMultiplier;
    Ok(pvzp_rs_game_speed_multiplier(&actualMultiplier));
    Check(actualMultiplier == exactMultiplier, "native speed scalar was rounded through f32");
    Ok(pvzp_rs_restore_game_speed());
    Check(gLawnApp->mUpdateMultiplier == multiplier, "exact multiplier restoration failed");
    Ok(pvzp_rs_set_game_speed(2));
    Ok(pvzp_rs_set_app_counter(777));
    Check(gLawnApp->mAppCounter == 777, "menu App clock was not applied");
    Ok(pvzp_rs_set_app_counter(clock));
    Ok(pvzp_rs_set_easy_planting_cheat(!planting));
    Ok(pvzp_rs_set_sun_cost_ignored(1));
    Ok(pvzp_rs_set_seed_recharge_ignored(1));
    Ok(pvzp_rs_set_plant_damage_rule(1));
    Ok(pvzp_rs_set_common_dance(1));
    Ok(pvzp_rs_set_cob_impact_delay(0));
    Check(pvzp_rs_sun_cost_ignored() && pvzp_rs_seed_recharge_ignored(), "menu rules were not applied");
    PvzpNative::RestoreAll();
    Check(gLawnApp->mFrameTime == frameTime && gLawnApp->mUpdateMultiplier == multiplier
        && gLawnApp->mEasyPlantingCheat == planting && !pvzp_rs_sun_cost_ignored()
        && !pvzp_rs_seed_recharge_ignored() && PvzpNative::gModifiers.plantDamageRule == 0,
        "menu settings did not restore");
    Ok(pvzp_rs_set_sun_cost_ignored(1)); // Must survive entry/reset until explicit cleanup.
    Ok(pvzp_rs_set_random_mode(1, 42)); // Construction temporarily unlocks, then restores this mode.
    Record("menu-rules");
}

static void ProfileChecks()
{
    const bool wasReadonly = PvzpNative::gModifiers.profileReadonly;
    PvzpNative::gModifiers.profileReadonly = false;
    std::uint32_t original;
    Ok(pvzp_rs_coins(&original));
    for (unsigned value : {0U, 99999U, 100000U, 999999999U}) {
        Ok(pvzp_rs_set_coins(value));
        unsigned actual;
        Ok(pvzp_rs_coins(&actual));
        Check(actual == value && gLawnApp->mPlayerInfo->mCoins == value, "coins are not native units");
    }
    Check(pvzp_rs_set_coins(0xffffffffU).code != 0, "invalid coins accepted");
    PvzpNative::gModifiers.profileReadonly = true;
    Check(pvzp_rs_set_coins(42).code != 0, "read-only profile was modified");
    PvzpNative::gModifiers.profileReadonly = false;
    Ok(pvzp_rs_set_coins(original));
    const auto oldLevel = gLawnApp->mPlayerInfo->mLevel;
    const auto oldPurchase = gLawnApp->mPlayerInfo->mPurchases[0];
    Ok(pvzp_rs_unlock_trophy());
    Check(gLawnApp->EarnedGoldTrophy(), "trophy criteria incomplete");
    Check(gLawnApp->mPlayerInfo->mLevel == oldLevel && gLawnApp->mPlayerInfo->mPurchases[0] == oldPurchase,
        "trophy modified unrelated profile fields");
    Check(pvzp_rs_unlock_hidden_modes().code != 0, "hidden page accepted without screen");
    Check(pvzp_rs_water_plant(0, 0).code != 0, "watering accepted outside garden");
    Check(pvzp_rs_mouse(0, -1, 0, 1).code != 0, "invalid mouse coordinate accepted");
    Ok(pvzp_rs_mouse(0, 20, 20, 1));
    Ok(pvzp_rs_mouse(1, 20, 20, 1));
    Ok(pvzp_rs_mouse(2, 20, 20, 1));
    PvzpNative::gModifiers.profileReadonly = wasReadonly;
    Record("profile-input");
}

static void AudioChecks()
{
    Check(pvzp_rs_sound(256, 0).code != 0, "invalid sound accepted");
    auto* manager = gLawnApp->mSoundManager.get();
    auto* first = manager->GetSoundInstance(0);
    auto* other = manager->GetSoundInstance(1);
    Check(first && other, "test sounds unavailable");
    Check(first->Play(true, false) && other->Play(true, false), "test playback failed");
    Ok(pvzp_rs_sound(0, 1));
    Check(!first->IsPlaying() && other->IsPlaying(), "stop_sound stopped wrong channels");
    other->Stop();
    first->Release();
    other->Release();
    Record("audio");
}

static void DanceChecks()
{
    auto* board = gLawnApp->mBoard;
    board->mZombieCountDown = 100000;
    PvzpNative::gModifiers.zombieSpawnStopped = true;
    const auto clock = gLawnApp->mAppCounter;
    Ok(pvzp_rs_set_fast_forward(1, 2, 1));
    board->mPaused = true;
    Step();
    Check(gLawnApp->mAppCounter == clock && !PvzpNative::gFastForward
        && !PvzpNative::gFastForwardSuppressWindow, "pause did not cancel fast-forward before advancing");
    board->mPaused = false;
    Ok(pvzp_rs_set_fast_forward(1, 2, 1));
    Step();
    Check(gLawnApp->mAppCounter == clock + 1, "nested fast-forward failed to use native update count");
    PvzpNative::gFastForward = false;
    PvzpNative::gFastForwardPerformance = 0;
    auto* normal = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
    auto* cone = board->AddZombieInRow(ZombieType::ZOMBIE_TRAFFIC_CONE, 1, 1);
    auto* bucket = board->AddZombieInRow(ZombieType::ZOMBIE_PAIL, 4, 1);
    auto* flag = board->AddZombieInRow(ZombieType::ZOMBIE_FLAG, 5, 1);
    auto* animation = gLawnApp->ReanimationTryToGet(normal->mBodyReanimID);
    const auto before = animation->mAnimTime;
    Ok(pvzp_rs_set_common_dance(1));
    Check(animation->mAnimTime == before, "dance setter eagerly changed animation");
    Step();
    Check(animation->mAnimTime == 0, "existing normal did not enter fast gait");
    Check(gLawnApp->ReanimationTryToGet(cone->mBodyReanimID)->mAnimTime == 0, "cone gait missing");
    Check(gLawnApp->ReanimationTryToGet(bucket->mBodyReanimID)->mAnimTime == 0, "bucket gait missing");
    Check(gLawnApp->ReanimationTryToGet(flag->mBodyReanimID)->mAnimTime != 0, "flag was affected by common gait");
    auto* newborn = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 1, 1);
    Step();
    Check(gLawnApp->ReanimationTryToGet(newborn->mBodyReanimID)->mAnimTime == 0, "newborn gait missing");
    Ok(pvzp_rs_set_common_dance(2));
    Step();
    const auto slowStart = animation->mFrameStart;
    Ok(pvzp_rs_set_common_dance(1));
    Step();
    Check(animation->mFrameStart != slowStart, "fast and slow use the same native animation");
    Ok(pvzp_rs_set_common_dance(0));
    Step();
    Check(animation->mAnimTime != 0, "none did not disable forced gait");
    const bool nativeBaseline=gLawnApp->mDanceMode;
    for (const bool enabled : {false,true,false,true}) {
        animation->mAnimTime=0.5f;
        Ok(pvzp_rs_set_dance_mode(enabled));
        std::uint8_t actual=0;Ok(pvzp_rs_dance_mode_enabled(&actual));
        Check(actual==enabled && board->mDanceMode==enabled,"native Dance flag differs");
        Check(animation->mAnimTime==0,"repeated native Dance failed to restart animation");
    }
    Ok(pvzp_rs_restore_dance_mode(nativeBaseline));
    for (auto* zombie : board->mZombies) zombie->DieNoLoot();
    Step();
    Record("dance");
}

static void CobChecks()
{
    auto* board = gLawnApp->mBoard;
    auto* cob = board->AddPlant(0, 0, SeedType::SEED_COBCANNON, SeedType::SEED_NONE);
    const auto cobId = board->mPlants.DataArrayGetID(cob);
    for (bool water : {false, true}) for (bool target : {false, true})
    for (bool fixed : {false, true}) for (bool delay : {false, true}) {
        for (auto* zombie : board->mZombies) zombie->DieNoLoot();
        Step();
        const int row = water ? 2 : 0;
        PvzpNative::gModifiers.cobFixedDelay = fixed;
        Ok(pvzp_rs_set_cob_impact_delay(delay));
        cob = board->mPlants.DataArrayTryToGet(cobId);
        Check(cob != nullptr, "test cob missing");
        cob->mState = PlantState::STATE_COBCANNON_READY;
        cob->CobCannonFire(600, board->GridToPixelY(7, row));
        cob->mShootingCounter = 2;
        for (int i = 0; i < 5 && board->mProjectiles.mSize == 0; ++i) Step();
        Check(board->mProjectiles.mSize == 1, "cob projectile not produced");
        auto* projectile = *board->mProjectiles.begin();
        const auto id = board->mProjectiles.DataArrayGetID(projectile);
        projectile->mProjectileAge = 30;
        projectile->mRow = row;
        projectile->mPosX = 600;
        projectile->mPosY = board->GridToPixelY(7, row);
        projectile->mPosZ = water ? -21.0f : -61.0f;
        projectile->mVelX = projectile->mVelY = projectile->mAccZ = 0;
        projectile->mVelZ = 1;
        projectile->mShadowY = projectile->mPosY + 67;
        projectile->mCobTargetRow = row;
        projectile->mCobTargetX = 600;
        if (target) {
            auto* zombie = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, row, 1);
            zombie->mPosX = 565;
            zombie->mX = 565;
            zombie->mPosY = board->GridToPixelY(7, row);
            zombie->mY = static_cast<int>(zombie->mPosY);
        }
        Step();
        projectile = board->mProjectiles.DataArrayTryToGet(id);
        Check(projectile && !projectile->mDead, "cob bypassed original height gate");
        Step();
        projectile = board->mProjectiles.DataArrayTryToGet(id);
        const bool immediate = !delay || fixed || target || water;
        Check(projectile && projectile->mDead == immediate, "cob impact gate combination mismatch");
        if (!immediate) {
            for (int i = 0; i < 20; ++i) Step();
            projectile = board->mProjectiles.DataArrayTryToGet(id);
            Check(!projectile || projectile->mDead, "natural no-target cob never landed");
        }
        Step();
    }
    PvzpNative::gModifiers.cobFixedDelay = false;
    Ok(pvzp_rs_set_cob_impact_delay(true));
    Record("cob-matrix");
}

static void SemanticActionChecks()
{
    auto* board = gLawnApp->mBoard;
    auto* world = reinterpret_cast<pvzp_rs_world*>(board);
    auto* cursor = board->mCursorObject.get();
    const auto savedCursor = *cursor;
    const auto savedBank = *board->mSeedBank;
    const auto savedMode = gLawnApp->mGameMode;
    const bool savedEasy = gLawnApp->mEasyPlantingCheat;
    auto* bank = board->mSeedBank.get();
    bank->mNumPackets = 3;
    for (int i = 0; i < 3; ++i) {
        bank->mSeedPackets[i].mIndex = i;
        bank->mSeedPackets[i].mPacketType = i == 0 ? SeedType::SEED_FLOWERPOT : SeedType::SEED_PEASHOOTER;
        bank->mSeedPackets[i].mActive = true;
    }
    cursor->mCursorType = CursorType::CURSOR_TYPE_PLANT_FROM_BANK;
    cursor->mSeedBankIndex = 1;
    gLawnApp->mEasyPlantingCheat = true;
    std::uint8_t usable = 1;
    Ok(pvzp_rs_seed_can_pick_up(&bank->mSeedPackets[1], &usable));
    Check(!usable, "semantic use stole a held packet with EasyPlanting enabled");
    gLawnApp->mGameMode = GameMode::GAMEMODE_CHALLENGE_COLUMN;
    Ok(pvzp_rs_world_has_conveyor(world, &usable));
    Check(usable, "column conveyor not detected");
    Ok(pvzp_rs_world_is_column_challenge(world, &usable));
    Check(usable, "column challenge not detected");
    Ok(pvzp_rs_seed_was_planted(&bank->mSeedPackets[0]));
    Check(cursor->mSeedBankIndex == 0 && !bank->mSeedPackets[0].mActive, "conveyor lost held packet identity");
    *bank = savedBank;
    gLawnApp->mGameMode = savedMode;
    gLawnApp->mEasyPlantingCheat = savedEasy;

    auto* cannon = board->NewPlant(0, 0, SeedType::SEED_COBCANNON, SeedType::SEED_NONE);
    cannon->mState = PlantState::STATE_COBCANNON_READY;
    cursor->mCursorType = CursorType::CURSOR_TYPE_COBCANNON_TARGET;
    cursor->mCobCannonPlantID = static_cast<PlantID>(board->mPlants.DataArrayGetID(cannon));
    Check(pvzp_rs_plant_fire_cob(cannon, 640, 120).code != 0 && cannon->mState == PlantState::STATE_COBCANNON_READY,
        "semantic fire consumed a user-selected cannon");
    cannon->Die();
    cursor->mCursorType = CursorType::CURSOR_TYPE_SHOVEL;

    auto* pea = board->NewPlant(7, 0, SeedType::SEED_PEASHOOTER, SeedType::SEED_NONE);
    auto* other = board->NewPlant(8, 0, SeedType::SEED_PEASHOOTER, SeedType::SEED_NONE);
    const auto tutorial = board->mTutorialState;
    const int count = board->mPlantsShoveled;
    board->mTutorialState = TutorialState::TUTORIAL_SHOVEL_DIG;
    Ok(pvzp_rs_plant_shovel(pea));
    Check(pea->mDead && board->mPlantsShoveled == count + 1 &&
        board->mTutorialState == TutorialState::TUTORIAL_SHOVEL_KEEP_DIGGING, "semantic shovel feedback missing");
    Ok(pvzp_rs_plant_shovel(pea));
    Check(board->mPlantsShoveled == count + 1, "dead shovel target counted twice");
    other->Die();
    board->mTutorialState = tutorial;

    const bool normal = pvzp_rs_normal_auto_collect_enabled() != 0;
    auto* dropped = board->AddCoin(300, 200, CoinType::COIN_USABLE_SEED_PACKET, CoinMotion::COIN_MOTION_FROM_SKY);
    dropped->mUsableSeedType = SeedType::SEED_PEASHOOTER;
    Ok(pvzp_rs_set_normal_auto_collect_enabled(1));
    Step();
    Check(!dropped->mIsBeingCollected && cursor->mCursorType == CursorType::CURSOR_TYPE_SHOVEL,
        "normal collection took over the cursor for a usable seed");
    Ok(pvzp_rs_set_normal_auto_collect_enabled(normal));
    dropped->mFadeCount = 1;
    Step();
    *cursor = savedCursor;
    Record("semantic-actions");
}

static void GardenAndHiddenChecks()
{
    Ok(pvzp_rs_back_to_main_menu());
    auto* player = gLawnApp->mPlayerInfo;
    const auto adventure = player->mFinishedAdventure;
    const auto level = player->mLevel;
    const auto mini = player->mHasUnlockedMinigames;
    const auto puzzle = player->mHasUnlockedPuzzleMode;
    const auto survival = player->mHasUnlockedSurvivalMode;
    player->mFinishedAdventure = 0;
    player->mLevel = 1;
    player->mHasUnlockedMinigames = player->mHasUnlockedPuzzleMode = player->mHasUnlockedSurvivalMode = 0;
    auto* selector = gLawnApp->mGameSelector;
    selector->SyncProfile(false);
    Check(selector->mMinigamesLocked && selector->mPuzzleLocked && selector->mSurvivalLocked, "temporary unlock baseline missing");
    Ok(pvzp_rs_set_temporary_unlock_enabled(1));
    Check(!selector->mMinigamesLocked && !selector->mPuzzleLocked && !selector->mSurvivalLocked, "temporary unlock did not refresh native menu");
    Check(player->mFinishedAdventure == 0 && player->mHasUnlockedMinigames == 0 && player->mLevel == 1, "temporary unlock changed profile");
    Ok(pvzp_rs_set_temporary_unlock_enabled(0));
    Check(selector->mMinigamesLocked && selector->mPuzzleLocked && selector->mSurvivalLocked, "temporary unlock did not restore native menu");
    player->mFinishedAdventure = adventure;
    player->mLevel = level;
    player->mHasUnlockedMinigames = mini;
    player->mHasUnlockedPuzzleMode = puzzle;
    player->mHasUnlockedSurvivalMode = survival;
    selector->SyncProfile(false);
    Check(selector != nullptr, "main menu missing");
    selector->mMinigamesLocked = false;
    Ok(pvzp_rs_set_hidden_pages_visible(1));
    selector->ButtonDepress(101);
    Check(gLawnApp->mChallengeScreen != nullptr, "challenge screen missing");
    Check(gLawnApp->mChallengeScreen->mPageButton[CHALLENGE_PAGE_LIMBO]->mVisible, "new hidden page button stayed hidden");
    Check(!gLawnApp->mChallengeScreen->mLimboPageUnlocked, "visibility rule changed cheat unlock state");
    Ok(pvzp_rs_set_hidden_pages_visible(0));
    Ok(pvzp_rs_unlock_hidden_modes());
    Check(gLawnApp->mChallengeScreen->mLimboPageUnlocked, "hidden page not opened");
    const auto cheatKeys = gLawnApp->mCheatKeys;
    Ok(pvzp_rs_unlock_hidden_modes());
    Check(gLawnApp->mCheatKeys == cheatKeys, "hidden page enabled all cheats");
    gLawnApp->KillChallengeScreen();
    auto* profile = gLawnApp->mPlayerInfo;
    profile->mNumPottedPlants = 1;
    profile->mPottedPlant[0] = {};
    auto& potted = profile->mPottedPlant[0];
    potted.mSeedType = SeedType::SEED_PEASHOOTER;
    potted.mWhichZenGarden = GardenType::GARDEN_MAIN;
    potted.mPlantAge = PottedPlantAge::PLANTAGE_SPROUT;
    potted.mFeedingsPerGrow = 3;
    gLawnApp->PreNewGame(GameMode::GAMEMODE_CHALLENGE_ZEN_GARDEN, false);
    auto* board = gLawnApp->mBoard;
    auto* garden = gLawnApp->mZenGarden;
    Check(board && garden && garden->mBoard == board, "garden not initialized");
    const auto coins = board->mCoins.mSize;
    Ok(pvzp_rs_water_plant(0, 0));
    Check(potted.mTimesFed == 1 && potted.mLastWateredTime != 0 && board->mCoins.mSize == coins + 1,
        "watering lost native side effects");
    Check(pvzp_rs_water_plant(0, 0).code != 0, "watering ignored native need cooldown");
    Check(pvzp_rs_water_plant(8, 8).code != 0, "invalid garden cell accepted");
    Record("garden-hidden");
}

static void BehaviorRuleChecks()
{
    auto* board = gLawnApp->mBoard;
    const bool cheatKeys = gLawnApp->mCheatKeys;
    gLawnApp->mCheatKeys = false;
    Ok(pvzp_rs_set_background_run_rule(2));
    Ok(pvzp_rs_set_pause_dialog_disabled(1));
    gLawnApp->LostFocus();
    Check(!board->mPaused && !gLawnApp->GetDialog(Dialogs::DIALOG_PAUSED), "pause-dialog gate did not block native focus pause");
    Ok(pvzp_rs_set_pause_dialog_disabled(0));
    gLawnApp->LostFocus();
    Check(board->mPaused && gLawnApp->GetDialog(Dialogs::DIALOG_PAUSED), "explicit background-off did not pause on focus loss");
    gLawnApp->KillDialog(Dialogs::DIALOG_PAUSED);
    Check(!board->mPaused, "native pause-dialog dismissal did not resume");
    for (int rule : {1, 0}) {
        Ok(pvzp_rs_set_background_run_rule(rule));
        gLawnApp->LostFocus();
        Check(!board->mPaused, "background-on/native desktop behavior was not retained");
    }
    gLawnApp->mCheatKeys = cheatKeys;
    for (int rule : {0, 1, 2}) {
        Ok(pvzp_rs_set_plant_special_countdown_rule(rule));
        for (int initial : {1, 2, 80}) {
            auto* cherry = board->AddPlant(0, 0, SeedType::SEED_CHERRYBOMB, SeedType::SEED_NONE);
            const auto plantId = board->mPlants.DataArrayGetID(cherry);
            cherry->mDoSpecialCountdown = initial;
            Step();
            auto* remaining = board->mPlants.DataArrayTryToGet(plantId);
            const bool dispatched = !remaining || remaining->mDead;
            const bool expected = rule == 0 ? initial == 1 : rule == 1 || initial != 1;
            Check(dispatched == expected, "plant countdown rule lost the PT countdown-one distinction");
            if (remaining && !remaining->mDead) remaining->Die();
        }
    }
    Ok(pvzp_rs_set_plant_special_countdown_rule(0));
    Check(pvzp_rs_set_plant_special_countdown_rule(3).code != 0, "invalid plant timing accepted");
    auto* walker = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
    walker->mPosX = 650;
    walker->mChilledCounter = 20;
    walker->mButteredCounter = 20;
    Ok(pvzp_rs_set_chilled_effects_disabled(1));
    Ok(pvzp_rs_set_butter_effects_disabled(1));
    Ok(pvzp_rs_set_zombie_walking_stopped(1));
    Step();
    Check(walker->mChilledCounter == 0 && walker->mButteredCounter == 0, "status rules did not run native countdown cleanup");
    Check(walker->mPosX == 650, "walking suppression did not preserve X");
    Ok(pvzp_rs_set_chilled_effects_disabled(0));
    Ok(pvzp_rs_set_butter_effects_disabled(0));
    Ok(pvzp_rs_set_zombie_walking_stopped(0));
    walker->mChilledCounter = 20;
    Step();
    Check(walker->mChilledCounter == 19 && walker->mPosX < 650, "normal countdown/walking did not resume");
    walker->DieNoLoot();

    auto* garg = board->AddZombieInRow(ZombieType::ZOMBIE_GARGANTUAR, 1, 1);
    garg->mPosX = 650;
    garg->mBodyHealth = garg->mBodyMaxHealth / 2 - 1;
    Ok(pvzp_rs_set_imp_throw_disabled(1));
    Step();
    Check(garg->mHasObject && garg->mZombiePhase != ZombiePhase::PHASE_GARGANTUAR_THROWING, "imp suppression allowed throw entry");
    Ok(pvzp_rs_set_imp_throw_disabled(0));
    Step();
    Check(garg->mZombiePhase == ZombiePhase::PHASE_GARGANTUAR_THROWING, "imp throw did not resume");
    garg->DieNoLoot();

    pvzp_rs_grid_item* crater;
    Ok(pvzp_rs_world_add_crater(reinterpret_cast<pvzp_rs_world*>(board), 4, 1, &crater));
    const auto id = board->mGridItems.DataArrayGetID(crater);
    Step();
    Check(board->mGridItems.DataArrayTryToGet(id) != nullptr, "ordinary crater disappeared early");
    Ok(pvzp_rs_set_craters_expire_immediately(1));
    Check(board->mGridItems.DataArrayTryToGet(id) != nullptr, "crater setter removed object synchronously");
    Step();
    auto* remaining = board->mGridItems.DataArrayTryToGet(id);
    Check(!remaining || remaining->mDead, "existing crater survived native expiration rule");
    Ok(pvzp_rs_set_craters_expire_immediately(0));

    auto* jack = board->AddZombieInRow(ZombieType::ZOMBIE_JACK_IN_THE_BOX, 0, 1);
    const auto jackId = board->mZombies.DataArrayGetID(jack);
    jack->mPosX = 650;
    jack->mPhaseCounter = 0;
    Ok(pvzp_rs_set_jack_explosion_rule(2));
    Step();
    Check(jack->mZombiePhase == ZombiePhase::PHASE_JACK_IN_THE_BOX_RUNNING, "never rule did not prevent opening");
    jack->mPhaseCounter = 123;
    Ok(pvzp_rs_set_jack_explosion_rule(1));
    Step();
    Check(jack->mZombiePhase == ZombiePhase::PHASE_JACK_IN_THE_BOX_POPPING && jack->mPhaseCounter == 110, "immediate rule must start the normal popping sequence");
    Ok(pvzp_rs_set_jack_explosion_rule(2));
    jack->mPhaseCounter = 1;
    Step();
    jack = board->mZombies.DataArrayTryToGet(jackId);
    Check(!jack || jack->mDead, "never rule incorrectly canceled an already opened jack");
    Ok(pvzp_rs_set_jack_explosion_rule(0));

    auto* pepper = board->AddZombieInRow(ZombieType::ZOMBIE_JALAPENO_HEAD, 0, 1);
    const auto pepperId = board->mZombies.DataArrayGetID(pepper);
    pepper->mPhaseCounter = 0;
    Ok(pvzp_rs_set_pepper_explosion_rule(2));
    Step();
    Check(!pepper->mDead, "never rule did not prevent jalapeno explosion");
    pepper->mPhaseCounter = 123;
    Ok(pvzp_rs_set_pepper_explosion_rule(1));
    Step();
    pepper = board->mZombies.DataArrayTryToGet(pepperId);
    Check(!pepper || pepper->mDead, "immediate jalapeno rule did not dispatch");
    Ok(pvzp_rs_set_pepper_explosion_rule(0));

    Ok(pvzp_rs_set_ice_trails_disabled(1));
    for (std::uint8_t row = 0; row < 6; ++row) Ok(pvzp_rs_set_ice_trail_countdown(row, 1));
    Check(pvzp_rs_set_ice_trail_countdown(6, 1).code != 0, "ice timer row overflow accepted");
    auto* zamboni = board->AddZombieInRow(ZombieType::ZOMBIE_ZAMBONI, 0, 1);
    zamboni->mPosX = 600;
    zamboni->mVelX = 0.2f;
    Step();
    Check(zamboni->mVelX == 0.2f, "PT no-ice rule must bypass the entire Zamboni update");
    for (int row = 0; row < 6; ++row) Check(board->mIceTimer[row] == 0, "existing ice did not expire");
    Ok(pvzp_rs_set_ice_trails_disabled(0));
    Step();
    Check(board->mIceTimer[0] > 0 && zamboni->mVelX != 0.2f, "Zamboni ice/speed update did not resume");
    zamboni->DieNoLoot();
}


static void ZombieStrengthChecks()
{
    auto* board = gLawnApp->mBoard;
    for (auto* zombie : board->mZombies) zombie->DieNoLoot();
    Step();
    Check(pvzp_rs_set_zombie_damage_rule(3).code != 0, "unknown zombie strength accepted");
    for (int rule : {0, 1, 2}) {
        Ok(pvzp_rs_set_zombie_damage_rule(rule));
        std::int32_t actual = -1;
        Ok(pvzp_rs_zombie_damage_rule(&actual));
        Check(actual == rule, "zombie strength readback differs");
        auto* body = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
        const int hp = body->mBodyHealth;
        body->TakeDamage(1, 0);
        Check(body->mBodyHealth == (rule == 0 ? hp - 1 : rule == 1 ? hp : 0), "body strength arithmetic differs");
        body->DieNoLoot();
        auto* helm = board->AddZombieInRow(ZombieType::ZOMBIE_PAIL, 0, 1);
        const int helmHp = helm->mHelmHealth;
        const int bodyHp = helm->mBodyHealth;
        helm->TakeDamage(1, 0);
        Check(helm->mHelmHealth == (rule == 0 ? helmHp - 1 : rule == 1 ? helmHp : 0), "helm strength differs");
        Check(helm->mBodyHealth == bodyHp, "weak helm incorrectly forwarded absorbed damage to body");
        helm->DieNoLoot();
        auto* shield = board->AddZombieInRow(ZombieType::ZOMBIE_DOOR, 0, 1);
        const int shieldHp = shield->mShieldHealth;
        const int shieldBodyHp = shield->mBodyHealth;
        shield->TakeDamage(1, 0);
        Check(shield->mShieldHealth == (rule == 0 ? shieldHp - 1 : rule == 1 ? shieldHp : 0), "shield strength differs");
        Check(shield->mBodyHealth == shieldBodyHp, "shield mode incorrectly forwarded remainder");
        shield->DieNoLoot();
        auto* burn = board->AddZombieInRow(ZombieType::ZOMBIE_GARGANTUAR, 0, 1);
        burn->mPosX = 600;
        burn->mX = 600;
        const int burnHp = burn->mBodyHealth;
        burn->ApplyBurn();
        Check(rule == 0 ? burn->mBodyHealth == burnHp - 1800 : rule == 1 ? burn->mBodyHealth == burnHp && !burn->IsDeadOrDying() : burn->IsDeadOrDying(), "burn branch differs");
        burn->DieNoLoot();
        auto* hypnotized = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
        hypnotized->StartMindControlled();
        Check(hypnotized->mMindControlled == (rule != 1) && hypnotized->mLastPortalX == -1, "mind-control flag or native side effects differ");
        hypnotized->DieNoLoot();
        auto* balloon = board->AddZombieInRow(ZombieType::ZOMBIE_BALLOON, 0, 1);
        auto* blover = board->NewPlant(0, 1, SeedType::SEED_BLOVER, SeedType::SEED_NONE);
        blover->BlowAwayFliers();
        Check(balloon->mBlowingAway == (rule != 1) && board->mFogBlownCountDown == 4000, "blover immunity changed fog or balloon behavior");
        balloon->DieNoLoot();
        blover->Die();
        auto* bitten = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
        bitten->mPosX = 560;
        bitten->mX = 560;
        auto* chomper = board->NewPlant(6, 0, SeedType::SEED_CHOMPER, SeedType::SEED_NONE);
        chomper->mState = PlantState::STATE_CHOMPER_BITING;
        chomper->mStateCountdown = 0;
        Check(chomper->FindTargetZombie(0, PlantWeapon::WEAPON_PRIMARY) == bitten, "chomper test target unavailable");
        chomper->UpdateChomper();
        Check(rule == 1 ? !bitten->mDead && chomper->mState == PlantState::STATE_CHOMPER_BITING_MISSED : bitten->mDead, "chomper must bite rather than swallow invincible zombies");
        bitten->DieNoLoot();
        chomper->Die();
        bool mowerChecked = false;
        for (auto* mower : board->mLawnMowers) {
            if (mower->mDead || mower->mRow != 0) continue;
            mowerChecked = true;
            const auto state = mower->mMowerState;
            const float x = mower->mPosX;
            const float y = mower->mPosY;
            auto* target = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
            target->mPosX = 510;
            target->mX = 510;
            mower->mPosX = 500;
            mower->mMowerState = LawnMowerState::MOWER_TRIGGERED;
            mower->Update();
            Check(rule == 1 ? !target->IsDeadOrDying() : target->IsDeadOrDying(), "mower immunity gate differs");
            mower->mPosX = x;
            mower->mPosY = y;
            mower->mMowerState = state;
            target->DieNoLoot();
            break;
        }
        Check(mowerChecked, "mower test object missing");
        Step();
    }
    Ok(pvzp_rs_set_zombie_damage_rule(0));
    Record("zombie-strength");
}

static void SeedRuleChecks()
{
    std::uint8_t visible=0;Ok(pvzp_rs_seed_bank_visible(&visible));
    const auto bankCount=gLawnApp->mBoard->mSeedBank->mNumPackets;
    Ok(pvzp_rs_set_seed_bank_visible(0));
    Check(!gLawnApp->mBoard->mSeedBank->mVisible,"bank did not hide");
    Check(gLawnApp->mBoard->mSeedBank->mNumPackets==bankCount,"hide changed bank content");
    Ok(pvzp_rs_set_seed_bank_visible(visible));
    auto* bank=gLawnApp->mBoard->mSeedBank.get();
    auto* world=reinterpret_cast<pvzp_rs_world*>(gLawnApp->mBoard);
    const auto width=bank->mWidth;
    for (std::uint8_t count=1;count<=10;++count) {
        Ok(pvzp_rs_world_set_seed_count(world,count));
        Check(bank->mNumPackets==count,"explicit seed count differs");
        Check(bank->mWidth>=width || count<bankCount,"seed bank width was not recomputed");
        for(unsigned index=1;index<count;++index)
            Check(bank->mSeedPackets[index].mX>bank->mSeedPackets[index-1].mX,"packet positions not recomputed");
    }
    const auto tenWidth=bank->mWidth;
    Check(pvzp_rs_world_set_seed_count(world,11).code!=0 && bank->mNumPackets==10 && bank->mWidth==tenWidth,"invalid count partially changed layout");
    pvzp_rs_seed* last=nullptr;Ok(pvzp_rs_world_seed_storage_at(world,9,&last));
    Check(last==&bank->mSeedPackets[9],"wrong packet storage pointer");
    Check(pvzp_rs_world_seed_storage_at(world,10,&last).code!=0,"out-of-range packet accepted");
    Ok(pvzp_rs_world_set_seed_count(world,static_cast<std::uint8_t>(bankCount)));
    auto* board = gLawnApp->mBoard;
    for (auto* plant : board->mPlants) if (!plant->mDead) plant->Die();
    const auto saved = board->mSeedBank->mSeedPackets[0];
    const int sun = board->mSunMoney;
    const bool easy = gLawnApp->mEasyPlantingCheat;
    auto* packet = &board->mSeedBank->mSeedPackets[0];
    packet->mPacketType = SeedType::SEED_GATLINGPEA;
    packet->mImitaterType = SeedType::SEED_NONE;
    packet->mActive = true;
    packet->mRefreshing = false;
    board->mSunMoney = 8000;
    gLawnApp->mEasyPlantingCheat = false;
    Ok(pvzp_rs_set_upgrade_planting_unrestricted(0));
    Check(!packet->CanPickUp(), "upgrade initially lacked its prerequisite");
    Check(board->CanPlantAt(7, 1, SeedType::SEED_GATLINGPEA) == PlantingReason::PLANTING_NEEDS_UPGRADE, "normal empty upgrade placement accepted");
    Ok(pvzp_rs_set_upgrade_planting_unrestricted(1));
    Check(packet->CanPickUp(), "PT upgrade rule did not bypass global prerequisites");
    Check(board->CanPlantAt(7, 1, SeedType::SEED_GATLINGPEA) == PlantingReason::PLANTING_OK, "PT empty upgrade gate not bypassed");
    auto* occupied = board->NewPlant(7, 1, SeedType::SEED_WALLNUT, SeedType::SEED_NONE);
    Check(board->CanPlantAt(7, 1, SeedType::SEED_GATLINGPEA) != PlantingReason::PLANTING_OK, "upgrade rule incorrectly bypassed occupied-square checks");
    occupied->Die();
    board->mSunMoney = 0;
    Check(!packet->CanPickUp(), "upgrade rule incorrectly removed sun cost");
    board->mSunMoney = 8000;
    packet->mActive = false;
    Ok(pvzp_rs_set_seed_recharge_shading_hidden(1));
    Check(!packet->CanPickUp(), "presentation rule changed packet availability");
    Ok(pvzp_rs_set_seed_recharge_shading_hidden(0));
    packet->mActive = true;
    Ok(pvzp_rs_set_upgrade_planting_unrestricted(0));
    Check(!packet->CanPickUp(), "upgrade prerequisites did not restore");
    Ok(pvzp_rs_set_seed_bank_topmost(1));
    Check(pvzp_rs_seed_bank_topmost() != 0, "seed-bank render rule readback differs");
    Ok(pvzp_rs_set_seed_bank_topmost(0));
    *packet = saved;
    board->mSunMoney = sun;
    gLawnApp->mEasyPlantingCheat = easy;
    Record("seed-rules");
}

static void CurrentSpawnChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    const auto count=board->mNumWaves;const auto seed=board->mBoardRandSeed;
    const auto mode=gLawnApp->mGameMode;const auto level=board->mLevel;const auto finished=gLawnApp->mPlayerInfo->mFinishedAdventure;
    const auto clock=board->mMainCounter;const auto wave=board->mCurrentWave;const auto remaining=board->mZombieCountDown;const auto start=board->mZombieCountDownStart;
    decltype(board->mZombiesInWave) saved;std::memcpy(saved,board->mZombiesInWave,sizeof(saved));
    bool allowed[NUM_ZOMBIE_TYPES];std::memcpy(allowed,board->mZombieAllowed,sizeof(allowed));
    for (int kind=0;kind<NUM_ZOMBIE_TYPES;++kind)board->mZombieAllowed[kind]=(kind==0||kind==1||kind==2||kind==4);
    for (const int n:{5,20,25,100}) {
        board->mNumWaves=n;
        for(auto& row:board->mZombiesInWave)std::fill(std::begin(row),std::end(row),ZombieType::ZOMBIE_IMP);
        Ok(pvzp_rs_set_random_mode(0,314159));const auto expected=Sexy::Rand();
        Ok(pvzp_rs_set_random_mode(0,314159));
        Ok(pvzp_rs_world_generate_current_spawn(world,12345));
        Check(Sexy::Rand()==expected,"natural generation consumed Battle RNG");
        Check(board->mNumWaves==n,"natural generation changed total waves");
        Check(board->mMainCounter==clock&&board->mCurrentWave==wave&&board->mZombieCountDown==remaining&&board->mZombieCountDownStart==start,"natural generation reset battle timing");
        if(n<MAX_ZOMBIE_WAVES)Check(board->mZombiesInWave[n][0]==ZombieType::ZOMBIE_IMP,"generation wrote beyond actual N waves");
        decltype(board->mZombiesInWave) first;std::memcpy(first,board->mZombiesInWave,sizeof(first));
        Ok(pvzp_rs_world_generate_current_spawn(world,12345));
        Check(std::memcmp(first,board->mZombiesInWave,sizeof(first))==0,"same native seed/context was not reproducible");
        for(int index=0;index<n;++index){std::uint8_t huge=0;Ok(pvzp_rs_world_spawn_huge_wave(world,index,&huge));Check(huge==((index+1)%10==0),"survival huge-wave fact differs");}
    }
    gLawnApp->mGameMode=GameMode::GAMEMODE_ADVENTURE;gLawnApp->mPlayerInfo->mFinishedAdventure=0;board->mLevel=2;board->mNumWaves=5;
    for(int index=0;index<5;++index){std::uint8_t huge=0;Ok(pvzp_rs_world_spawn_huge_wave(world,index,&huge));Check(huge==(index==4),"short Adventure huge-wave fact differs");}
    board->mLevel=1;std::uint8_t huge=1;Ok(pvzp_rs_world_spawn_huge_wave(world,4,&huge));Check(!huge,"first Adventure level wrongly has a huge wave");
    gLawnApp->mGameMode=mode;board->mLevel=level;gLawnApp->mPlayerInfo->mFinishedAdventure=finished;
    board->mNumWaves=count;board->mBoardRandSeed=seed;std::memcpy(board->mZombieAllowed,allowed,sizeof(allowed));std::memcpy(board->mZombiesInWave,saved,sizeof(saved));
    Record("spawn-current");
}

static void ChallengeGraveChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    auto* target=board->AddPlant(6,0,SeedType::SEED_WALLNUT,SeedType::SEED_NONE);
    auto* neighbor=board->AddPlant(7,0,SeedType::SEED_WALLNUT,SeedType::SEED_NONE);
    const bool enabled=board->mEnableGraveStones;board->mEnableGraveStones=false;
    const auto particles=gLawnApp->mEffectSystem->mParticleHolder->mParticleSystems.mSize;
    Ok(pvzp_rs_world_spawn_challenge_grave(world,0,6));
    Check(target->mDead&&!neighbor->mDead,"challenge grave did not remove exactly the target cell plants");
    Check(board->mEnableGraveStones,"challenge grave did not enable graves");
    Check(gLawnApp->mEffectSystem->mParticleHolder->mParticleSystems.mSize>particles,"challenge grave did not create native rise particles");
    GridItem* found=nullptr;
    for(auto* item:board->mGridItems)if(!item->mDead&&item->mGridItemType==GridItemType::GRIDITEM_GRAVESTONE&&item->mGridX==6&&item->mGridY==0)found=item;
    Check(found!=nullptr,"challenge grave did not create the native item");
    const auto occupied=board->mGridItems.mSize;
    Check(pvzp_rs_world_spawn_challenge_grave(world,-1,7).code!=0&&!neighbor->mDead&&board->mGridItems.mSize==occupied,"invalid grave coordinates caused partial mutation");
    found->GridItemDie();neighbor->Die();board->mEnableGraveStones=enabled;
    Record("challenge-grave");
}

static void RakeChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    auto& stock=gLawnApp->mPlayerInfo->mPurchases[StoreItem::STORE_ITEM_RAKE];const auto savedStock=stock;
    const auto savedMode=gLawnApp->mGameMode;
    std::vector<GridItem*> before;for(auto* item:board->mGridItems)if(!item->mDead)before.push_back(item);
    stock=0;const auto count=board->mGridItems.mSize;board->PlaceRake();
    Check(board->mGridItems.mSize==count,"ordinary rake ignored empty stock");
    std::uint8_t placed=0;Ok(pvzp_rs_world_place_rake(world,1,3,&placed));
    Check(placed&&stock==0,"explicit rake consumed stock or failed");
    GridItem* found=nullptr;
    for(auto* item:board->mGridItems)if(!item->mDead&&item->mGridItemType==GridItemType::GRIDITEM_RAKE&&item->mGridY==1&&item->mGridX==3)found=item;
    Check(found&&found->mGridItemState==GridItemState::GRIDITEM_STATE_RAKE_ATTRACTING,"rake native attracting state missing");
    Check(gLawnApp->ReanimationTryToGet(found->mGridItemReanimID)!=nullptr,"rake native animation missing");
    stock=2;board->PlaceRake();Check(stock==1,"ordinary rake did not consume exactly one stock");
    const auto occupied=board->mGridItems.mSize;
    Check(pvzp_rs_world_place_rake(world,6,3,&placed).code!=0&&stock==1&&board->mGridItems.mSize==occupied,"invalid rake partially changed the game");
    gLawnApp->mGameMode=GameMode::GAMEMODE_CHALLENGE_BOBSLED_BONANZA;
    Ok(pvzp_rs_world_place_rake(world,0,0,&placed));Check(!placed&&stock==1,"forced rake bypassed a native mode guard");
    gLawnApp->mGameMode=savedMode;stock=savedStock;
    for(auto* item:board->mGridItems)if(!item->mDead&&std::find(before.begin(),before.end(),item)==before.end())item->GridItemDie();
    Record("rake");
}

static void LayoutAndReclaimChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    const auto scene=board->mBackground;const auto clock=board->mMainCounter;const auto wave=board->mCurrentWave;
    const auto stage=board->mChallenge->mSurvivalStage;
    std::vector<unsigned> mowers;for(auto* mower:board->mLawnMowers)if(!mower->mDead)mowers.push_back(board->mLawnMowers.DataArrayGetID(mower));
    auto* survivor=board->AddPlant(0,0,SeedType::SEED_WALLNUT,SeedType::SEED_NONE);
    for(int value=0;value<=9;++value) {
        Ok(pvzp_rs_world_set_scene_layout(world,value));
        Check(static_cast<int>(board->mBackground)==value&&!survivor->mDead,"layout change cleared a live object");
        Check(board->mMainCounter==clock&&board->mCurrentWave==wave&&board->mChallenge->mSurvivalStage==stage,"layout change reset progression");
        for(auto id:mowers)Check(board->mLawnMowers.DataArrayTryToGet(id)!=nullptr,"layout change replaced an existing mower");
        Check(board->mPlantRow[2]==((value==2||value==3)?PlantRowType::PLANTROW_POOL:PlantRowType::PLANTROW_NORMAL),"layout terrain differs");
    }
    Check(pvzp_rs_world_set_scene_layout(world,10).code!=0&&static_cast<int>(board->mBackground)==9,"invalid layout changed background");
    Ok(pvzp_rs_world_set_scene_layout(world,static_cast<int>(scene)));
    auto* dead=board->AddPlant(1,0,SeedType::SEED_WALLNUT,SeedType::SEED_NONE);const auto deadId=board->mPlants.DataArrayGetID(dead);
    dead->Die();const auto occupied=board->mPlants.mSize;
    Ok(pvzp_rs_world_process_delete_queue(world));
    Check(board->mPlants.mSize<occupied&&board->mPlants.DataArrayTryToGet(deadId)==nullptr&&!survivor->mDead,"native delete queue did not reclaim only dead objects");
    Check(board->mMainCounter==clock,"reclamation advanced game time");
    auto* mine=board->AddPlant(2,0,SeedType::SEED_POTATOMINE,SeedType::SEED_NONE);
    Ok(pvzp_rs_plant_setup_izombie(mine));
    auto* animation=gLawnApp->ReanimationTryToGet(mine->mBodyReanimID);
    Check(mine->mState==PlantState::STATE_POTATO_ARMED&&mine->mBlinkCountdown==0&&animation&&animation->mAnimRate==0,"I-Zombie setup did not arm/freeze the native plant");
    mine->Die();survivor->Die();
    Record("layout-reclaim-izombie");
}

static void MowerChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    const auto mode=gLawnApp->mGameMode;const auto scene=board->mBackground;
    const auto bonus=board->mBonusLawnMowersRemaining;const auto clock=board->mMainCounter;
    board->mBonusLawnMowersRemaining=0;
    for(int background=0;background<=5;++background) {
        Ok(pvzp_rs_world_clear_lawn_mowers(world));
        Ok(pvzp_rs_world_process_delete_queue(world));
        Ok(pvzp_rs_world_set_scene_layout(world,background));
        gLawnApp->mGameMode=GameMode::GAMEMODE_CHALLENGE_LAST_STAND;
        board->InitLawnMowers();
        Check(board->mLawnMowers.mSize==0,"ordinary mower mode guards changed");
        Ok(pvzp_rs_world_initialize_mowers_ready(world));
        const int expected=(background==2||background==3)?6:5;
        Check(board->mLawnMowers.mSize==expected,"ready mower rows differ");
        for(auto* mower:board->mLawnMowers) {
            Check(!mower->mDead&&mower->mVisible&&mower->mPosX==-21.0f&&mower->mMowerState==LawnMowerState::MOWER_READY,"ready mower state/position differs");
            Check(mower->mPosY==board->GetPosYBasedOnRow(19.0f,mower->mRow)+23.0f,"ready mower Y used roll-in X");
            Check(pvzp_rs_mower_id(mower)==board->mLawnMowers.DataArrayGetID(mower),"mower ID does not name native pool");
            Ok(pvzp_rs_mower_start(mower));
            Check(mower->mMowerState==LawnMowerState::MOWER_TRIGGERED,"native mower start did not trigger");
        }
    }
    Ok(pvzp_rs_world_clear_lawn_mowers(world));Ok(pvzp_rs_world_process_delete_queue(world));
    Ok(pvzp_rs_world_set_scene_layout(world,0));gLawnApp->mGameMode=mode;
    board->InitLawnMowers();
    Check(board->mLawnMowers.mSize==5,"ordinary five-row initialization changed");
    LawnMower* first=nullptr;
    for(auto* mower:board->mLawnMowers) {
        Check(!mower->mVisible&&mower->mPosX==-160.0f,"ordinary mower defaults changed");
        if(!first)first=mower;
    }
    board->mBonusLawnMowersRemaining=2;const auto occupied=board->mLawnMowers.mSize;
    Ok(pvzp_rs_mower_die(first));
    Check(first->mDead&&board->mLawnMowers.mSize==occupied+1&&board->mBonusLawnMowersRemaining==1,"native mower death suppressed its bonus replacement");
    bool rolling=false;for(auto* mower:board->mLawnMowers)if(!mower->mDead&&mower->mRow==first->mRow)rolling=mower->mMowerState==LawnMowerState::MOWER_ROLLING_IN&&mower->mPosX==-160.0f;
    Check(rolling,"bonus replacement did not preserve native roll-in behavior");
    board->mBonusLawnMowersRemaining=0;Ok(pvzp_rs_world_clear_lawn_mowers(world));Ok(pvzp_rs_world_process_delete_queue(world));
    // Exactly five free native slots are enough for a five-row initialization.
    // Fill with real initialized one-row mowers; never forge pool metadata.
    for(int row=0;row<6;++row)board->mPlantRow[row]=row==0?PlantRowType::PLANTROW_NORMAL:PlantRowType::PLANTROW_DIRT;
    while(board->mLawnMowers.mMaxSize-board->mLawnMowers.mSize>5)Ok(pvzp_rs_world_initialize_mowers_ready(world));
    for(int row=0;row<5;++row)board->mPlantRow[row]=PlantRowType::PLANTROW_NORMAL;
    Ok(pvzp_rs_world_initialize_mowers_ready(world));
    Check(board->mLawnMowers.mSize==board->mLawnMowers.mMaxSize,"exact-capacity initialization refused");
    auto it=board->mLawnMowers.begin();auto* atAward=*it;++it;auto* withBonus=*it;
    const auto award=board->mLevelAwardSpawned;board->mLevelAwardSpawned=true;board->mBonusLawnMowersRemaining=2;
    Ok(pvzp_rs_mower_die(atAward));Check(atAward->mDead&&board->mBonusLawnMowersRemaining==2,"award state falsely required a bonus slot");
    board->mLevelAwardSpawned=false;
    Check(pvzp_rs_mower_die(withBonus).code!=0&&!withBonus->mDead&&board->mBonusLawnMowersRemaining==2,"full pool was changed before rejecting bonus allocation");
    Ok(pvzp_rs_world_process_delete_queue(world));Ok(pvzp_rs_mower_die(withBonus));
    Check(withBonus->mDead&&board->mBonusLawnMowersRemaining==1,"freed bonus slot was not used");
    board->mLevelAwardSpawned=award;
    board->mBonusLawnMowersRemaining=0;Ok(pvzp_rs_world_clear_lawn_mowers(world));Ok(pvzp_rs_world_process_delete_queue(world));
    Ok(pvzp_rs_world_set_scene_layout(world,static_cast<int>(scene)));Ok(pvzp_rs_world_initialize_mowers_ready(world));
    board->mBonusLawnMowersRemaining=bonus;
    Check(board->mMainCounter==clock,"mower operations advanced native time");
    Record("mowers");
}

static void ZombieCapacityChecks()
{
    auto* board=gLawnApp->mBoard;auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    for(auto* zombie:board->mZombies)if(!zombie->mDead)zombie->DieNoLoot();
    Ok(pvzp_rs_world_process_delete_queue(world));
    const auto clock=board->mMainCounter;
    std::vector<Zombie*> fillers;
    while(board->mZombies.mMaxSize-board->mZombies.mSize>3) {
        auto* zombie=board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL,0,Zombie::ZOMBIE_WAVE_UI);
        Check(zombie!=nullptr,"capacity fixture could not allocate a real UI zombie");fillers.push_back(zombie);
        // Occupied dead slots are a real same-frame case. Reclaim only effects
        // so the independent 1024-animation pool does not fail before this test
        // reaches the zombie-pool boundary; no allocator metadata is forged.
        zombie->DieNoLoot();gLawnApp->mEffectSystem->ProcessDeleteQueue();
    }
    const auto count=board->mZombies.mSize;
    Zombie* result=nullptr;
    Ok(pvzp_rs_world_add_zombie_in_row(world,ZombieType::ZOMBIE_BOBSLED,0,0,&result));
    Check(!result&&board->mZombies.mSize==count,"three free slots incorrectly accepted a four-member team");
    Check(pvzp_rs_world_place_zombie(world,ZombieType::ZOMBIE_BOBSLED,0,0,&result).code!=0&&board->mZombies.mSize==count,"grid bobsled failed after mutation");
    Ok(pvzp_rs_world_add_zombie_in_row(world,ZombieType::ZOMBIE_BOBSLED,0,Zombie::ZOMBIE_WAVE_UI,&result));
    Check(result&&board->mZombies.mSize==count+1,"UI bobsled should remain a single native object");
    result->DieNoLoot();gLawnApp->mEffectSystem->ProcessDeleteQueue();
    board->mZombies.DataArrayFree(result);
    board->mZombies.DataArrayFree(fillers.back());fillers.pop_back();
    const auto before=board->mZombies.mSize;
    Ok(pvzp_rs_world_place_zombie(world,ZombieType::ZOMBIE_BOBSLED,0,0,&result));
    Check(result&&board->mZombies.mSize==before+4,"four free slots did not create the exact native team");
    for(auto* zombie:board->mZombies)if(!zombie->mDead)zombie->DieNoLoot();
    Ok(pvzp_rs_world_process_delete_queue(world));
    Check(board->mZombies.mSize==0&&board->mMainCounter==clock,"capacity test did not reclaim without stepping");
    Record("zombie-capacity");
}

static void PlantSlotChecks()
{
    auto* board=gLawnApp->mBoard;
    auto* world=reinterpret_cast<pvzp_rs_world*>(board);
    for(auto* plant:board->mPlants) if(!plant->mDead) plant->Die();
    Step();
    auto* source=board->NewPlant(3,0,SeedType::SEED_WALLNUT,SeedType::SEED_NONE);
    auto* target=board->NewPlant(4,1,SeedType::SEED_SUNFLOWER,SeedType::SEED_NONE);
    const unsigned sourceId=board->mPlants.DataArrayGetID(source), targetId=board->mPlants.DataArrayGetID(target);
    const unsigned sourceSlot=sourceId&65535U,targetSlot=targetId&65535U;
    source->mPlantHealth=1234;source->mStateCountdown=456;source->mLaunchCounter=789;
    const Plant before=*source;
    const auto body=source->mBodyReanimID;
    auto* bodyPointer=gLawnApp->ReanimationTryToGet(body);
    auto* bungee=board->AddZombieInRow(ZombieType::ZOMBIE_BUNGEE,0,1);
    bungee->mTargetPlantID=static_cast<PlantID>(sourceId);
    const auto cursor=*board->mCursorObject;
    board->mCursorObject->mGlovePlantID=static_cast<PlantID>(sourceId);
    board->mCursorObject->mDuplicatorPlantID=static_cast<PlantID>(targetId);
    board->mCursorObject->mCobCannonPlantID=static_cast<PlantID>(sourceId);
    unsigned moved=0,swapped=0;
    const unsigned nextKey=board->mPlants.mNextKey;
    Ok(pvzp_rs_world_relocate_plant_slot(world,sourceId,targetSlot,targetId,&moved,&swapped, 0));
    Check(board->mPlants.mNextKey==nextKey && board->mPlants.DataArrayGetID(source)==sourceId
        && board->mPlants.DataArrayGetID(target)==targetId && std::memcmp(source,&before,sizeof(Plant))==0,
        "dry-run changed payload, IDs or allocator");
    Check(pvzp_rs_world_relocate_plant_slot(world,sourceId,targetSlot,0,&moved,&swapped, 1).code!=0,"unapproved occupied target was overwritten");
    Check(board->mPlants.mNextKey==nextKey && source->mPlantHealth==1234,"rejected move mutated state");
    Ok(pvzp_rs_world_relocate_plant_slot(world,sourceId,targetSlot,targetId,&moved,&swapped, 1));
    Check((moved&65535U)==targetSlot && (swapped&65535U)==sourceSlot,"swap slots differ");
    Check(!board->mPlants.DataArrayTryToGet(sourceId)&&!board->mPlants.DataArrayTryToGet(targetId),"swap retained stale IDs");
    source=board->mPlants.DataArrayTryToGet(moved);
    Check(source && std::memcmp(source,&before,sizeof(Plant))==0,"swap changed plant payload");
    Check(gLawnApp->ReanimationTryToGet(body)==bodyPointer,"swap recreated animation");
    Check(static_cast<unsigned>(bungee->mTargetPlantID)==moved && static_cast<unsigned>(board->mCursorObject->mGlovePlantID)==moved
        && static_cast<unsigned>(board->mCursorObject->mDuplicatorPlantID)==swapped && static_cast<unsigned>(board->mCursorObject->mCobCannonPlantID)==moved,"native plant references not remapped");
    const unsigned high=board->mPlants.mMaxSize-1;
    unsigned highId=0,unused=0;
    Ok(pvzp_rs_world_relocate_plant_slot(world,moved,high,0,&highId,&unused, 1));
    Check(board->mPlants.mSize==2 && board->mPlants.mMaxUsedCount==high+1,"move broke allocation counters");
    Check(!board->mPlants.DataArrayTryToGet(moved),"empty move retained old identity");
    unsigned back=0;
    Ok(pvzp_rs_world_relocate_plant_slot(world,highId,targetSlot,0,&back,&unused, 1));
    Check(back!=moved && std::memcmp(board->mPlants.DataArrayTryToGet(back),&before,sizeof(Plant))==0,"A-B-A resurrected an ID or changed state");
    source=board->mPlants.DataArrayTryToGet(back);
    source->Die();board->mPlants.DataArrayTryToGet(swapped)->Die();bungee->DieNoLoot();
    *board->mCursorObject=cursor;
    Step();
    Check(!gLawnApp->ReanimationTryToGet(body),"moved animation did not release with its owner");
    auto* probe=board->NewPlant(1,1,SeedType::SEED_PEASHOOTER,SeedType::SEED_NONE);
    Check(probe && board->mPlants.mSize==1,"allocator unusable after relocation cleanup");
    probe->Die();Step();
    Record("plant-slots");
}

static int Update(std::uint8_t, std::uint64_t rounds)
{
    ++dispatchCount;
    if (dispatchCount == 1) Record("first-update");
    try {
        observedRounds += rounds;
        Check(rounds <= 1 && observedRounds <= 1, "round delta repeated");
        if (phase == 0) {
            if (!gLawnApp->mPlayerInfo || (gLawnApp->mTitleScreen && !gLawnApp->mTitleScreen->mLoadingThreadComplete)) return 0;
            MenuChecks();
            PvzpNative::gModifiers.profileReadonly = true;
            ProfileChecks();
            AudioChecks();
            Ok(pvzp_rs_enter_endless(2));
            std::uint8_t clicked;
            Ok(pvzp_rs_click_continue_dialog_if_present(&clicked));
            Ok(pvzp_rs_world_reset(reinterpret_cast<pvzp_rs_world*>(gLawnApp->mBoard), 0, 123, 8000, 1));
            Check(PvzpNative::gRandomStreams[0].locked && PvzpNative::gRandomStreams[1].locked
                && PvzpNative::gRandomStreams[0].fixed == 42 && PvzpNative::gRandomStreams[1].seed == 123,
                "reset did not restore locked random streams");
            Ok(pvzp_rs_set_random_mode(0, 123));
            phase = 1;
            return 0;
        }
        if (phase == 1) {
            if (!gLawnApp->mSeedChooserScreen || !gLawnApp->mSeedChooserScreen->mMouseVisible) return 0;
            Ok(pvzp_rs_start_battle());
            phase = 2;
            return 0;
        }
        if (phase == 2) {
            if (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING) return 0;
            Check(gLawnApp->mBoard->mMainCounter == 0
                && gLawnApp->mAppCounter == static_cast<int>(PvzpNative::gPendingDancerClock),
                "first playing origin was not normalized");
            Check(pvzp_rs_sun_cost_ignored(), "menu rule was lost during entry/reset");
            Ok(pvzp_rs_set_sun_cost_ignored(0));
            pausedAppClock = gLawnApp->mAppCounter;
            pausedBoardClock = gLawnApp->mBoard->mMainCounter;
            Ok(pvzp_rs_set_advanced_pause(1, 0, 0, 0, 0));
            phase = 20;
            return 0;
        }
        if (phase == 20) {
            Check(gLawnApp->mAppCounter == pausedAppClock && gLawnApp->mBoard->mMainCounter == pausedBoardClock,
                "advanced pause advanced native clocks");
            Ok(pvzp_rs_set_advanced_pause(0, 0, 0, 0, 0));
            Record("pause");
            DanceChecks();
            CobChecks();
            SemanticActionChecks();
            BehaviorRuleChecks();
            ZombieStrengthChecks();
            SeedRuleChecks();
            PlantSlotChecks();
            CurrentSpawnChecks();
            ChallengeGraveChecks();
            RakeChecks();
            LayoutAndReclaimChecks();
            MowerChecks();
            ZombieCapacityChecks();
            auto* board = gLawnApp->mBoard;
            auto* plant = board->AddPlant(4, 0, SeedType::SEED_WALLNUT, SeedType::SEED_NONE);
            auto* zombie = board->AddZombieInRow(ZombieType::ZOMBIE_NORMAL, 0, 1);
            const auto rectangle = plant->GetPlantRect();
            zombie->mPosX = static_cast<float>(rectangle.mX - zombie->mZombieAttackRect.mX + 1);
            zombie->mX = static_cast<int>(zombie->mPosX);
            zombie->mZombieAge = 3 /* native TICKS_BETWEEN_EATS is 4 */;
            bitePlant = board->mPlants.DataArrayGetID(plant);
            biter = board->mZombies.DataArrayGetID(zombie);
            biteHealth = plant->mPlantHealth;
            RegisterBattle(1U << 1);
            phase = 50;
            return 0;
        }
        if (phase == 50) {
            auto* plant = gLawnApp->mBoard->mPlants.DataArrayTryToGet(bitePlant);
            if (biteAttempts != 1 || biteFinished != 1 || !plant || plant->mPlantHealth != biteHealth) {
                auto* zombie = gLawnApp->mBoard->mZombies.DataArrayTryToGet(biter);
                char detail[256];
                std::snprintf(detail, sizeof(detail), "bite details hp=%d before=%d attempts=%d finished=%d outcome=%d applied=%d phase=%d eating=%d frames=%d",
                    plant ? plant->mPlantHealth : -1, biteHealth, biteAttempts, biteFinished, biteOutcome, biteApplied,
                    zombie ? int(zombie->mZombiePhase) : -1, zombie && zombie->mIsEating, frameBegins);
                Record(detail);
            }
            Check(plant && plant->mPlantHealth == biteHealth && biteAttempts == 1 && biteFinished == 1
                && biteOutcome == 0 && biteApplied == 0, "native bite suppression/result mismatch");
            suppressBite = false;
            gLawnApp->mBoard->mZombies.DataArrayTryToGet(biter)->mZombieAge = 3 /* native TICKS_BETWEEN_EATS is 4 */;
            phase = 51;
            return 0;
        }
        if (phase == 51) {
            auto* plant = gLawnApp->mBoard->mPlants.DataArrayTryToGet(bitePlant);
            Check(plant && plant->mPlantHealth == biteHealth - biteRequested && biteAttempts == 2
                && biteFinished == 2 && biteOutcome == 2 && biteApplied == biteRequested,
                "native bite damage/result mismatch");
            PvzpPlugin::ClearBattleCallbacks();
            gLawnApp->mBoard->mZombies.DataArrayTryToGet(biter)->DieNoLoot();
            frameBegins = frameEnds = 0;
            Record("event-suppression");
            RegisterBattle();
            phase = 3;
            return 0;
        }
        if (phase == 3) {
            Check(frameBegins == 1 && frameEnds == 1, "first fight frame not paired");
            firstEpoch = eventEpoch;
            PvzpPlugin::ClearBattleCallbacks(); // ExitFight, even though Board will survive.
            gLawnApp->mBoard->mLevelComplete = true;
            phase = 4;
            return 0;
        }
        if (phase == 4) {
            Check(frameBegins == 1 && frameEnds == 1, "stale sink ran after ExitFight");
            Check(PvzpNative::gBoardEpoch == firstEpoch, "survival stage unexpectedly replaced Board");
            if (!gLawnApp->mSeedChooserScreen || !gLawnApp->mSeedChooserScreen->mMouseVisible) return 0;
            Ok(pvzp_rs_start_battle());
            phase = 5;
            return 0;
        }
        if (phase == 5) {
            if (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING) return 0;
            Check(observedRounds == 1, "completed round was never delivered");
            RegisterBattle(); // EnterFight on the same Board.
            phase = 6;
            return 0;
        }
        if (phase == 6) {
            Check(frameBegins == 2 && frameEnds == 2 && eventEpoch == firstEpoch, "same-Board sink not rebound");
            const auto before = destroyedBoards;
            Ok(pvzp_rs_world_reset(reinterpret_cast<pvzp_rs_world*>(gLawnApp->mBoard), 0, 321, 8000, 1));
            Check(destroyedBoards == before + 1 && destroyWasRevoked && gLawnApp->mPlugin.battleInterest == 0,
                "Board destruction did not revoke battle callbacks first");
            phase = 7;
            return 0;
        }
        if (phase == 7) {
            Check(frameBegins == 2 && frameEnds == 2, "new Board inherited old sink");
            if (!gLawnApp->mSeedChooserScreen || !gLawnApp->mSeedChooserScreen->mMouseVisible) return 0;
            Ok(pvzp_rs_start_battle());
            phase = 8;
            return 0;
        }
        if (phase == 8) {
            if (gLawnApp->mGameScene != GameScenes::SCENE_PLAYING) return 0;
            Check(gLawnApp->mBoard->mMainCounter == 0
                && gLawnApp->mAppCounter == static_cast<int>(PvzpNative::gPendingDancerClock),
                "second reset origin was not normalized");
            RegisterBattle();
            phase = 9;
            return 0;
        }
        if (phase == 9) {
            Check(frameBegins == 3 && frameEnds == 3 && eventEpoch != firstEpoch, "new Board sink not rebound");
            PvzpPlugin::ClearBattleCallbacks();
            Record("sink-lifecycle");
            GardenAndHiddenChecks();
            Record("passed");
            phase = 10;
            PvzpPlugin::RequestStop();
        }
    } catch (const std::exception& error) {
        Record(error.what());
        PvzpPlugin::RequestStop();
    }
    return 0;
}

extern "C" __declspec(dllexport) std::uint32_t pvzp_plugin_abi_version() { return PvzpPlugin::AbiVersion; }
extern "C" __declspec(dllexport) std::int32_t pvzp_plugin_initialize()
{
    Record("initialize");
    if (!pvzp_rs_validate_abi()) return 1;
    auto entry = PvzpPlugin::Layout[0];
    if (!pvzp_rs_check_raw_layout(&entry, 1) || pvzp_rs_check_raw_layout(nullptr, 1)) return 1;
    entry.value ^= 1;
    if (pvzp_rs_check_raw_layout(&entry, 1)) return 1;
    entry.name = 0;
    if (pvzp_rs_check_raw_layout(&entry, 1)) return 1;
    Record("validated");
    return PvzpPlugin::SetUpdateCallback(Update) && PvzpPlugin::SetBoardDestroyingCallback(Destroyed) ? 0 : 2;
}
extern "C" __declspec(dllexport) std::int32_t pvzp_plugin_shutdown() { Record("shutdown"); return 0; }
BOOL WINAPI DllMain(HINSTANCE, DWORD reason, LPVOID reserved)
{
    if (reason == DLL_PROCESS_ATTACH) Record("attach");
    if (reason == DLL_PROCESS_DETACH && !reserved) Record("unload");
    return TRUE;
}
